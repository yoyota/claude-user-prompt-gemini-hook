use chrono::Local;
use clap::{error::ErrorKind, Parser};
use serde_json::{json, Value};
use std::{
    error::Error,
    fmt, fs,
    io::{self, Read},
    path::Path,
};

#[derive(Parser)]
#[command(about = "Gemini hook for Claude Code")]
struct Cli {
    #[arg(long)]
    system_instruction_file: Option<String>,

    /// Gemini API key
    #[arg(long, env = "GEMINI_API_KEY")]
    gemini_api_key: Option<String>,

    /// Gemini model to use
    #[arg(long, default_value = "gemini-3.8-flash")]
    model: String,

    /// Fallback Gemini model to use when primary is overloaded
    #[arg(long, default_value = "gemini-3.5-flash-lite")]
    fallback_model: String,

    /// Directory to save log files (optional; skipped if path does not exist)
    #[arg(long)]
    log_dir: Option<String>,
}

#[derive(Debug)]
enum GeminiError {
    Overloaded,
    Other(Box<dyn std::error::Error>),
}

impl fmt::Display for GeminiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeminiError::Overloaded => write!(f, "Gemini model overloaded"),
            GeminiError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl Error for GeminiError {}

impl GeminiError {
    fn other(e: impl Into<Box<dyn Error>>) -> Self {
        Self::Other(e.into())
    }
}

/// A hook failure must never block the user's prompt. Claude Code treats exit
/// code 2 as "block" (clap's usage-error code), so every error is reported as a
/// non-blocking `systemMessage` and the process always exits 0.
fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            e.exit()
        }
        Err(e) => return report_error(&usage_error_summary(&e)),
    };
    if let Err(e) = run(&cli) {
        report_error(&e.to_string());
    }
}

/// clap renders a usage error as "error: ..." followed by usage and hint lines;
/// keep only the first non-blank line, without the "error: " prefix.
fn usage_error_summary(e: &clap::Error) -> String {
    let rendered = e.to_string();
    let line = rendered
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(&rendered);
    line.trim_start_matches("error: ").to_string()
}

fn report_error(message: &str) {
    eprintln!("gemini_hook: {message}");
    let notice = format!("gemini_hook failed, prompt sent without proofreading: {message}");
    println!("{}", json!({ "systemMessage": notice }));
}

fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let hook: Value =
        serde_json::from_str(&input).map_err(|e| format!("invalid hook JSON on stdin: {e}"))?;

    let prompt = hook["prompt"]
        .as_str()
        .ok_or("missing or non-string prompt field")?;

    let effective_prompt = match classify_prompt(prompt) {
        PromptAction::Skip => return Ok(()),
        PromptAction::Process(p) => p,
    };

    let api_key = cli
        .gemini_api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or("GEMINI_API_KEY is not set (env var or --gemini-api-key)")?;

    let system_instruction = cli
        .system_instruction_file
        .as_deref()
        .map(|path| {
            fs::read_to_string(path)
                .map_err(|e| format!("cannot read system instruction file {path}: {e}"))
        })
        .transpose()?;

    let message = call_gemini_with_retry(&cli.model, &cli.fallback_model, |model| {
        call_gemini(
            api_key,
            model,
            system_instruction.as_deref(),
            effective_prompt,
        )
    })?;
    if let (Some(log_dir), Some(session_id)) = (cli.log_dir.as_deref(), hook["session_id"].as_str())
    {
        save_log(log_dir, session_id, &message)?;
    }
    println!("{}", json!({ "systemMessage": message }));
    Ok(())
}

enum PromptAction<'a> {
    Process(&'a str),
    Skip,
}

fn classify_prompt(prompt: &str) -> PromptAction<'_> {
    let prompt = prompt.trim();
    if !prompt.starts_with('/') {
        return if prompt.contains(char::is_whitespace) {
            PromptAction::Process(prompt)
        } else {
            PromptAction::Skip
        };
    }
    // Find the first whitespace after the command token
    let Some(pos) = prompt.find(char::is_whitespace) else {
        return PromptAction::Skip;
    };
    let rest = prompt[pos..].trim_start();
    if rest.is_empty() {
        PromptAction::Skip
    } else {
        PromptAction::Process(rest)
    }
}

fn save_log(log_dir: &str, session_id: &str, message: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(log_dir);
    if !path.exists() {
        return Ok(());
    }
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let file = path.join(format!("{timestamp}_{session_id}.md"));
    fs::write(&file, message)
        .map_err(|e| format!("failed to write log to {}: {e}", file.display()))?;
    Ok(())
}

fn call_gemini_with_retry(
    model: &str,
    fallback_model: &str,
    call: impl Fn(&str) -> Result<String, GeminiError>,
) -> Result<String, GeminiError> {
    match call(model) {
        Err(GeminiError::Overloaded) => call(fallback_model),
        result => result,
    }
}

fn call_gemini(
    api_key: &str,
    model: &str,
    system_instruction: Option<&str>,
    prompt: &str,
) -> Result<String, GeminiError> {
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let body = build_request_body(prompt, system_instruction);

    let http_response = ureq::post(&url)
        .set("x-goog-api-key", api_key)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(classify_ureq_error)?;

    let response: Value = http_response.into_json().map_err(GeminiError::other)?;
    let text = parse_gemini_response(&response).map_err(GeminiError::other)?;

    Ok(format_response(text, model))
}

fn build_request_body(prompt: &str, system_instruction: Option<&str>) -> Value {
    let mut body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }]
    });
    if let Some(instruction) = system_instruction {
        body["systemInstruction"] = json!({ "parts": [{ "text": instruction }] });
    }
    body
}

fn parse_gemini_response(response: &Value) -> Result<&str, &'static str> {
    response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or("unexpected response shape")
}

fn format_response(text: &str, model: &str) -> String {
    let date_now = Local::now().format("%Y-%m-%d");
    format!("{text}\n\n## Connections\n\n- Model: [[{model}]]\n- Date: [[{date_now}]]\n")
}

fn classify_ureq_error(e: ureq::Error) -> GeminiError {
    let ureq::Error::Status(code, resp) = e else {
        return GeminiError::other(e);
    };
    if code == 503 {
        return GeminiError::Overloaded;
    }
    let body = resp.into_string().unwrap_or_default();
    let message = extract_api_error_message(&body);
    GeminiError::other(format!("HTTP {code}: {message}"))
}

fn extract_api_error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.to_string())
}

#[cfg(test)]
mod tests;

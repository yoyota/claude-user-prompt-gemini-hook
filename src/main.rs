use chrono::Utc;
use clap::Parser;
use serde_json::{json, Value};
use std::{
    error::Error,
    fmt, fs,
    io::{self, Read},
    path::Path,
    process,
};

#[derive(Parser)]
#[command(about = "Gemini hook for Claude Code")]
struct Cli {
    #[arg(long)]
    system_instruction_file: Option<String>,

    /// Gemini API key
    #[arg(long, env = "GEMINI_API_KEY")]
    gemini_api_key: String,

    /// Gemini model to use
    #[arg(long, default_value = "gemini-3.1-flash-lite-preview")]
    model: String,

    /// Fallback Gemini model to use when primary is overloaded
    #[arg(long, default_value = "gemma-4-26b")]
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

fn main() {
    let cli = Cli::parse();

    let system_instruction = cli
        .system_instruction_file
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok());

    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    let hook: Value = match serde_json::from_str(&input) {
        Ok(h) => h,
        Err(_) => return,
    };

    let Some(prompt) = hook["prompt"].as_str() else {
        eprintln!("Missing or non-string prompt field");
        return;
    };
    let prompt = prompt.trim();

    let effective_prompt = match classify_prompt(prompt) {
        PromptAction::Skip => return,
        PromptAction::Process(p) => p,
    };

    let result = call_gemini_with_retry(&cli.model, &cli.fallback_model, |model| {
        call_gemini(
            &cli.gemini_api_key,
            model,
            system_instruction.as_deref(),
            effective_prompt,
        )
    });
    let (message, is_err) = match result {
        Ok(s) => (s, false),
        Err(e) => (e.to_string(), true),
    };
    println!(
        "{}",
        json!({ "suppressOutput": false, "systemMessage": message })
    );
    if let (Some(log_dir), Some(session_id)) = (cli.log_dir.as_deref(), hook["session_id"].as_str())
    {
        save_log(log_dir, session_id, &message);
    }
    if is_err {
        process::exit(1);
    }
}

enum PromptAction<'a> {
    Process(&'a str),
    Skip,
}

fn classify_prompt(prompt: &str) -> PromptAction<'_> {
    if !prompt.starts_with('/') {
        return PromptAction::Process(prompt);
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

fn save_log(log_dir: &str, session_id: &str, message: &str) {
    let path = Path::new(log_dir);
    if !path.is_dir() {
        return;
    }
    let timestamp = Utc::now().format("%Y_%m_%d_%H_%M_%S");
    let filename = format!("{timestamp}_{session_id}.md");
    let _ = fs::write(path.join(filename), message);
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

fn extract_api_error_message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.to_string())
}

fn call_gemini(
    api_key: &str,
    model: &str,
    system_instruction: Option<&str>,
    prompt: &str,
) -> Result<String, GeminiError> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = build_request_body(prompt, system_instruction);

    let http_response = ureq::post(&url)
        .set("x-goog-api-key", api_key)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(classify_ureq_error)?;

    let response: Value = http_response
        .into_json()
        .map_err(|e| GeminiError::Other(e.into()))?;
    let text = parse_gemini_response(&response).map_err(|e| GeminiError::Other(e.into()))?;

    Ok(format_response(prompt, text, model))
}

fn classify_ureq_error(e: ureq::Error) -> GeminiError {
    let ureq::Error::Status(code, resp) = e else {
        return GeminiError::Other(Box::new(e));
    };
    if code == 503 {
        return GeminiError::Overloaded;
    }
    let body = resp.into_string().unwrap_or_default();
    let message = extract_api_error_message(&body);
    GeminiError::Other(format!("HTTP {code}: {message}").into())
}

fn format_response(prompt: &str, text: &str, model: &str) -> String {
    format!("## User:\n\n{prompt}\n\n## Gemini:\n\n>model: {model}\n\n{text}\n\n")
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

#[cfg(test)]
mod tests;

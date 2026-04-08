use chrono::Utc;
use clap::Parser;
use serde_json::{json, Value};
use std::{
    fs,
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

    /// Directory to save log files (optional; skipped if path does not exist)
    #[arg(long)]
    log_dir: Option<String>,
}

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

    let result = call_gemini(
        &cli.gemini_api_key,
        &cli.model,
        system_instruction.as_deref(),
        effective_prompt,
    );
    let is_err = result.is_err();
    let message = match result {
        Ok(s) => s,
        Err(e) => e.to_string(),
    };
    println!(
        "{}",
        json!({ "suppressOutput": false, "systemMessage": message })
    );
    if let (Some(log_dir), Some(session_id)) =
        (cli.log_dir.as_deref(), hook["session_id"].as_str())
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
        body["systemInstruction"] =
            json!({ "parts": [{ "text": instruction }] });
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
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );

    let body = build_request_body(prompt, system_instruction);

    let http_response = ureq::post(&url)
        .set("x-goog-api-key", api_key)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| -> Box<dyn std::error::Error> {
            let ureq::Error::Status(code, resp) = e else {
                return Box::new(e);
            };
            let body = resp.into_string().unwrap_or_default();
            let message = extract_api_error_message(&body);
            format!("HTTP {code}: {message}").into()
        })?;

    let response: Value = http_response.into_json()?;
    let text = parse_gemini_response(&response)?;

    Ok(format!("\n### User:\n\n{}\n\n{}", prompt, text))
}

#[cfg(test)]
mod tests;

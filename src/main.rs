use clap::Parser;
use serde_json::{json, Value};
use std::process;
use std::{
    fs,
    io::{self, Read},
};

#[derive(Parser)]
#[command(about = "Gemini proofreading hook for Claude Code")]
struct Cli {
    #[arg(long)]
    system_instruction_file: Option<String>,

    /// Gemini API key
    #[arg(long, env = "GEMINI_API_KEY")]
    gemini_api_key: String,
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
        eprintln!("Prompt is empty");
        return;
    };
    let prompt = prompt.trim();

    if prompt.starts_with('/') {
        return;
    }

    let (message, is_err) = match call_gemini(
        &cli.gemini_api_key,
        system_instruction.as_deref(),
        prompt,
    ) {
        Ok(result) => (result, false),
        Err(e) => (e.to_string(), true),
    };
    println!(
        "\n{}",
        json!({ "suppressOutput": false, "systemMessage": message })
    );
    if is_err {
        process::exit(1);
    }
}

fn call_gemini(
    api_key: &str,
    system_instruction: Option<&str>,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // let model = "gemini-3-flash-preview";
    let model = "gemini-3.1-flash-lite-preview";
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",model);

    let mut body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }]
    });
    if let Some(instruction) = system_instruction {
        body["systemInstruction"] =
            json!({ "parts": [{ "text": instruction }] });
    }

    let http_response = ureq::post(&url)
        .set("x-goog-api-key", api_key)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| match e {
            ureq::Error::Status(code, resp) => {
                let body = resp.into_string().unwrap_or_default();
                let message = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| {
                        v["error"]["message"].as_str().map(|s| s.to_string())
                    })
                    .unwrap_or(body);
                format!("HTTP {code}: {message}").into()
            }
            other => Box::new(other) as Box<dyn std::error::Error>,
        })?;

    let response: Value = http_response.into_json()?;

    let text = response["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or("unexpected response shape")?
        .to_string();

    Ok(text)
}

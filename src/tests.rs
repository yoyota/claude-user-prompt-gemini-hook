use super::*;
use serde_json::json;

// --- build_request_body ---

#[test]
fn body_prompt_only_has_no_system_instruction() {
    let body = build_request_body("hello", None);
    assert!(body.get("systemInstruction").is_none());
}

#[test]
fn body_with_system_instruction_includes_it() {
    let body = build_request_body("hello", Some("be helpful"));
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        json!("be helpful")
    );
}

#[test]
fn body_prompt_text_is_set() {
    let body = build_request_body("my prompt", None);
    assert_eq!(body["contents"][0]["parts"][0]["text"], json!("my prompt"));
}

// --- parse_gemini_response ---

#[test]
fn parse_valid_response_returns_text() {
    let resp = json!({"candidates": [{"content": {"parts": [{"text": "hello"}]}}]});
    assert_eq!(parse_gemini_response(&resp), Ok("hello"));
}

#[test]
fn parse_missing_candidates_key_returns_err() {
    let resp = json!({});
    assert!(parse_gemini_response(&resp).is_err());
}

#[test]
fn parse_empty_candidates_array_returns_err() {
    let resp = json!({"candidates": []});
    assert!(parse_gemini_response(&resp).is_err());
}

#[test]
fn parse_text_is_number_returns_err() {
    let resp = json!({"candidates": [{"content": {"parts": [{"text": 42}]}}]});
    assert!(parse_gemini_response(&resp).is_err());
}

#[test]
fn parse_missing_parts_key_returns_err() {
    let resp = json!({"candidates": [{"content": {}}]});
    assert!(parse_gemini_response(&resp).is_err());
}

// --- extract_api_error_message ---

#[test]
fn extract_nested_error_message() {
    let body = r#"{"error": {"message": "API key invalid"}}"#;
    assert_eq!(extract_api_error_message(body), "API key invalid");
}

#[test]
fn extract_falls_back_to_body_when_no_message_field() {
    let body = r#"{"status": "error"}"#;
    assert_eq!(extract_api_error_message(body), body);
}

#[test]
fn extract_non_json_body_returned_verbatim() {
    let body = "internal server error";
    assert_eq!(extract_api_error_message(body), body);
}

// --- save_log ---

#[test]
fn save_log_nonexistent_dir_is_noop() {
    save_log("/tmp/nonexistent_dir_xyz_123456", "sess1", "hello");
}

#[test]
fn save_log_creates_file_in_existing_dir() {
    let dir = tempfile::tempdir().unwrap();
    save_log(dir.path().to_str().unwrap(), "sess1", "hello");
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn save_log_filename_starts_with_session_id() {
    let dir = tempfile::tempdir().unwrap();
    save_log(dir.path().to_str().unwrap(), "mysession", "hello");
    let entry = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let filename = entry.file_name().into_string().unwrap();
    assert!(filename.starts_with("mysession_"));
}

#[test]
fn save_log_file_content_matches_message() {
    let dir = tempfile::tempdir().unwrap();
    save_log(dir.path().to_str().unwrap(), "sess1", "my log content");
    let entry = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let content = std::fs::read_to_string(entry.path()).unwrap();
    assert_eq!(content, "my log content");
}

// --- classify_prompt ---

#[test]
fn classify_normal_prose_returns_process_with_full_string() {
    // Happy path: normal prose prompt -> Process with the full original string
    let prompt = "fix the bug in main.rs";
    assert!(matches!(
        classify_prompt(prompt),
        PromptAction::Process(p) if p == prompt
    ));
}

#[test]
fn classify_slash_command_single_word_params_returns_process() {
    // Happy path: /feature auth -> Process("auth")
    assert!(matches!(
        classify_prompt("/feature auth"),
        PromptAction::Process("auth")
    ));
}

#[test]
fn classify_slash_command_multi_word_params_returns_process() {
    // Happy path: /feature implement auth -> Process("implement auth")
    assert!(matches!(
        classify_prompt("/feature implement auth"),
        PromptAction::Process("implement auth")
    ));
}

#[test]
fn classify_slash_command_multiple_internal_spaces_preserves_internal_spaces() {
    // Happy path: /cmd  foo  bar -> Process with leading whitespace stripped,
    // internal spaces preserved
    let result = classify_prompt("/cmd  foo  bar");
    assert!(matches!(result, PromptAction::Process(p) if p == "foo  bar"));
}

#[test]
fn classify_bare_slash_command_no_space_returns_skip() {
    // Edge case: /help (no space, no params) -> Skip
    assert!(matches!(classify_prompt("/help"), PromptAction::Skip));
}

#[test]
fn classify_slash_command_only_whitespace_after_returns_skip() {
    // Edge case: /help   (followed only by whitespace) -> Skip
    assert!(matches!(classify_prompt("/help   "), PromptAction::Skip));
}

#[test]
fn classify_empty_string_returns_process_empty() {
    // Edge case: empty string -> Process("") (no leading slash, existing behavior)
    assert!(matches!(classify_prompt(""), PromptAction::Process("")));
}

#[test]
fn classify_slash_command_long_param_returns_only_param_portion() {
    // Edge case: long param string -> Process returns only the param portion, not the command word
    let result = classify_prompt("/refactor please rewrite this entire module to be idiomatic Rust");
    assert!(matches!(
        result,
        PromptAction::Process(p)
            if p == "please rewrite this entire module to be idiomatic Rust"
    ));
}

#[test]
fn classify_bare_slash_returns_skip() {
    // Edge case: prompt is exactly "/" -> Skip
    assert!(matches!(classify_prompt("/"), PromptAction::Skip));
}

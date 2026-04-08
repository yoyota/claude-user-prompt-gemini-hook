use super::*;
use serde_json::json;

// --- should_skip_prompt ---

#[test]
fn skip_slash_command() {
    assert!(should_skip_prompt("/help"));
}

#[test]
fn keep_normal_prompt() {
    assert!(!should_skip_prompt("fix the bug in main.rs"));
}

#[test]
fn keep_empty_prompt() {
    assert!(!should_skip_prompt(""));
}

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

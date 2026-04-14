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
fn save_log_filename_ends_with_session_id() {
    let dir = tempfile::tempdir().unwrap();
    save_log(dir.path().to_str().unwrap(), "mysession", "hello");
    let entry = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let filename = entry.file_name().into_string().unwrap();
    assert!(
        filename.ends_with("_mysession.md"),
        "Value mismatch! Current filename is: {}",
        filename
    );
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
    let prompt = "fix the bug in main.rs";
    assert!(matches!(
        classify_prompt(prompt),
        PromptAction::Process(p) if p == prompt
    ));
}

#[test]
fn classify_slash_command_single_word_params_returns_process() {
    assert!(matches!(
        classify_prompt("/feature auth"),
        PromptAction::Process("auth")
    ));
}

#[test]
fn classify_slash_command_multi_word_params_returns_process() {
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
    let result =
        classify_prompt("/refactor please rewrite this entire module to be idiomatic Rust");
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

// --- GeminiError ---

mod gemini_error {
    use super::*;

    #[test]
    fn overloaded_variant_exists_and_matches() {
        // GeminiError::Overloaded must exist and be matchable
        let err = GeminiError::Overloaded;
        assert!(matches!(err, GeminiError::Overloaded));
    }

    #[test]
    fn other_variant_exists_and_wraps_error() {
        // GeminiError::Other must exist, accept a Box<dyn Error>, and be matchable
        let inner: Box<dyn std::error::Error> = "some error".into();
        let err = GeminiError::Other(inner);
        assert!(matches!(err, GeminiError::Other(_)));
    }

    #[test]
    fn overloaded_does_not_match_other() {
        let err = GeminiError::Overloaded;
        assert!(!matches!(err, GeminiError::Other(_)));
    }

    #[test]
    fn other_does_not_match_overloaded() {
        let inner: Box<dyn std::error::Error> = "boom".into();
        let err = GeminiError::Other(inner);
        assert!(!matches!(err, GeminiError::Overloaded));
    }
}

// --- format_response ---

#[test]
fn format_response_contains_prompt_text_and_model() {
    let result = format_response("Here is the fix.", "gemini-flash");
    assert!(result.contains("Here is the fix."));
    assert!(result.contains("gemini-flash"));
}

#[test]
fn format_response_sections_appear_in_order() {
    let result = format_response("my answer", "my-model");
    let answer_pos = result.find("my answer").unwrap();
    let model_pos = result.find("my-model").unwrap();
    assert!(answer_pos < model_pos);
}

mod call_gemini_with_retry_tests {
    use super::*;

    #[test]
    fn primary_model_called_first_on_success() {
        use std::cell::Cell;
        use std::rc::Rc;

        let call_count = Rc::new(Cell::new(0u32));
        let call_count_clone = Rc::clone(&call_count);

        let result = call_gemini_with_retry("primary-model", "fallback-model", |model| {
            call_count_clone.set(call_count_clone.get() + 1);
            assert_eq!(
                model, "primary-model",
                "fallback must not be called on success"
            );
            Ok("primary response".to_string())
        });

        assert_eq!(result.unwrap(), "primary response");
        assert_eq!(call_count.get(), 1, "caller must be invoked exactly once");
    }

    #[test]
    fn fallback_model_called_when_primary_returns_overloaded() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let models_called: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(vec![]));
        let models_clone = Rc::clone(&models_called);

        let result = call_gemini_with_retry("primary-model", "fallback-model", |model| {
            models_clone.borrow_mut().push(model.to_string());
            if model == "primary-model" {
                Err(GeminiError::Overloaded)
            } else {
                Ok(format!("fallback response from {model}"))
            }
        });

        assert_eq!(result.unwrap(), "fallback response from fallback-model");
        let calls = models_called.borrow();
        assert_eq!(calls.as_slice(), ["primary-model", "fallback-model"]);
    }

    #[test]
    fn non_503_error_from_primary_fails_immediately_without_calling_fallback() {
        use std::cell::Cell;
        use std::rc::Rc;

        let fallback_call_count = Rc::new(Cell::new(0u32));
        let fallback_clone = Rc::clone(&fallback_call_count);

        let result = call_gemini_with_retry("primary-model", "fallback-model", |model| {
            if model == "primary-model" {
                Err(GeminiError::Other("HTTP 400: bad request".into()))
            } else {
                fallback_clone.set(fallback_clone.get() + 1);
                Ok("should not reach here".to_string())
            }
        });

        assert!(result.is_err());
        assert_eq!(
            fallback_call_count.get(),
            0,
            "fallback must not be called on non-503 errors"
        );
    }

    #[test]
    fn error_propagated_when_fallback_also_fails() {
        let result = call_gemini_with_retry("primary", "fallback", |model| {
            if model == "primary" {
                Err(GeminiError::Overloaded)
            } else {
                Err(GeminiError::Other("fallback also failed".into()))
            }
        });

        assert!(result.is_err());
    }

    #[test]
    fn default_fallback_model_in_cli_is_gemini_3_flash_preview() {
        // The --fallback-model CLI argument must default to "gemini-3-flash-preview".
        // We verify the default by parsing an args list that omits --fallback-model.
        // Cli::try_parse_from is used to avoid process::exit on parse failure.
        let cli = Cli::try_parse_from(["gemini_hook", "--gemini-api-key", "test-key"])
            .expect("parse must succeed with only required args");

        assert_eq!(
            cli.fallback_model, "gemini-3-flash-preview",
            "--fallback-model default must be \"gemini-3-flash-preview\""
        );
    }
}

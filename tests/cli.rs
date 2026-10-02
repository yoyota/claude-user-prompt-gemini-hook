//! Runs the built `gemini_hook` binary as a subprocess, the way Claude Code does.

use std::io::{ErrorKind, Write};
use std::process::{Command, Output, Stdio};

// Cargo builds the binary before integration tests and passes its path here.
const BIN: &str = env!("CARGO_BIN_EXE_gemini_hook");

// Runs the binary with `args` and `stdin`, with GEMINI_API_KEY removed from the env.
fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .env_remove("GEMINI_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"));
    // On a usage error the binary exits without reading stdin, so the pipe may be closed.
    if let Err(e) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(
            e.kind(),
            ErrorKind::BrokenPipe,
            "failed to write stdin: {e}"
        );
    }
    child.wait_with_output().expect("failed to wait for child")
}

fn notice(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"));
    v["systemMessage"].as_str().unwrap_or_default().to_string()
}

#[test]
fn missing_api_key_exits_zero_with_notice() {
    let output = run(&[], r#"{"prompt": "fix the bug", "session_id": "s"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(notice(&output).contains("GEMINI_API_KEY"), "{output:?}");
}

#[test]
fn missing_api_key_is_ignored_for_skipped_prompt() {
    let output = run(&[], r#"{"prompt": "yes"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "{output:?}");
}

#[test]
fn unknown_argument_exits_zero_not_two() {
    let output = run(&["--no-such-flag"], r#"{"prompt": "fix the bug"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(notice(&output).contains("--no-such-flag"), "{output:?}");
}

#[test]
fn missing_system_instruction_file_exits_zero_with_notice() {
    let output = run(
        &[
            "--gemini-api-key",
            "dummy",
            "--system-instruction-file",
            "/nonexistent/x.md",
        ],
        r#"{"prompt": "fix the bug"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(notice(&output).contains("/nonexistent/x.md"), "{output:?}");
}

#[test]
fn help_still_works() {
    let output = run(&["--help"], "");
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage"));
}

#[test]
fn invalid_json_on_stdin_exits_zero_and_reports_parse_failure() {
    let output = run(&["--gemini-api-key", "dummy"], "not json");
    assert_eq!(output.status.code(), Some(0));
    assert!(notice(&output).contains("JSON"), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("JSON"),
        "{output:?}"
    );
}

#[test]
fn empty_stdin_exits_zero_and_reports_parse_failure() {
    let output = run(&["--gemini-api-key", "dummy"], "");
    assert_eq!(output.status.code(), Some(0));
    assert!(notice(&output).contains("JSON"), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("JSON"),
        "{output:?}"
    );
}

# Plan: Slash-Command Parameter Extraction

## 1. Requirements

### Must Do
- If a prompt starts with `/` and the command token is followed by non-whitespace parameter text, extract that parameter text and send it to Gemini as the effective prompt.
- If a prompt starts with `/` and there is no parameter text after the command token (empty or whitespace only), skip the prompt entirely — do not call Gemini.
- All existing behavior for prompts that do NOT start with `/` must remain unchanged.

### Must NOT Do
- Must not alter any other field of the JSON hook output beyond `systemMessage`.
- Must not pass the raw slash-command string (e.g., `/feature implement auth`) as the prompt to Gemini; only the extracted parameter text is sent.
- Must not modify `call_gemini`, `build_request_body`, `parse_gemini_response`, `extract_api_error_message`, or `save_log` — this change is isolated to prompt classification and `main`.

---

## 2. Interface Design

### Replace `should_skip_prompt` with `classify_prompt`

```rust
pub enum PromptAction<'a> {
    /// Send this text to Gemini (either the original prompt or extracted params).
    Process(&'a str),
    /// Do not call Gemini; exit silently.
    Skip,
}

fn classify_prompt(prompt: &str) -> PromptAction<'_>;
```

**Input contract:** `prompt` is already `.trim()`-ed before being passed in (matches the existing `let prompt = prompt.trim()` in `main`).

**Output contract:**

| Input shape | Result |
|---|---|
| Does not start with `/` | `Process(prompt)` — full original string |
| Starts with `/`, no tokens after the command | `Skip` |
| Starts with `/`, has parameter text after the command | `Process(params)` where `params` is the substring after the command token, `.trim_start()`-ed |

"Command token" is defined as the first whitespace-delimited word (the word starting with `/`). "Parameter text" is everything after that first word, with leading whitespace stripped. If what remains is empty (or the prompt has no space at all), the result is `Skip`.

### Call-site change in `main`

The existing block:

```rust
if should_skip_prompt(prompt) {
    return;
}
// ... call_gemini(... prompt ...)
```

becomes:

```rust
let effective_prompt = match classify_prompt(prompt) {
    PromptAction::Skip => return,
    PromptAction::Process(p) => p,
};
// ... call_gemini(... effective_prompt ...)
```

No other changes to `main` are needed.

### Error cases

`classify_prompt` is total and infallible — it returns a `PromptAction` for every input. There are no error paths.

---

## 3. Test Scenarios

All tests belong in `src/tests.rs` alongside the existing suite, under a `// --- classify_prompt ---` section.

- [ ] Happy path: normal prose prompt → `Process` with the full original string
- [ ] Happy path: slash-command with single-word params (`/feature auth`) → `Process("auth")`
- [ ] Happy path: slash-command with multi-word params (`/feature implement auth`) → `Process("implement auth")`
- [ ] Happy path: slash-command with multiple internal spaces in params (`/cmd  foo  bar`) → `Process` with leading whitespace stripped, internal spaces preserved
- [ ] Edge case: bare slash-command, no space, no params (`/help`) → `Skip`
- [ ] Edge case: slash-command followed only by whitespace (`/help   `) → `Skip` (trimmed param text is empty)
- [ ] Edge case: empty string → `Process("")` (no leading slash, existing behavior unchanged)
- [ ] Edge case: prompt starting with `/` that has a long param string → `Process` returns only the param portion, not the command word
- [ ] Edge case: prompt is exactly `/` → `Skip`
- [ ] Regression: existing `skip_slash_command`, `keep_normal_prompt`, `keep_empty_prompt` tests must be rewritten against `classify_prompt` rather than deleted, preserving their intent

---

## 4. Out of Scope

- Recognising or validating specific command names (e.g., `/feature`, `/help`). The classifier is command-agnostic.
- Changing how the formatted Gemini response is rendered (the `### User:` markdown block in `call_gemini`).
- Adding a deny-list or allow-list of slash commands.
- Any change to CLI arguments, logging, or the JSON output structure.
- Multi-word command tokens (a command is always the single first whitespace-delimited word).

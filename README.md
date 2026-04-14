# gemini-proofread-hook

A Claude Code hook that proofreads your prompts using Gemini before Claude sees them,
injecting corrections as a system message into Claude's context.

## TLDR

In this example, the hook is configured as an **English coaching assistant** — it gives grammar
feedback and natural phrasing suggestions before Claude processes the request. You can swap in
any system instruction you like (proofreading, translation, tone adjustment, etc.).

```text
❯ Write python hello world program

  ⎿  UserPromptSubmit says:

     ### User:

     Write python hello world program

     ---

     As an English language coach, I will help you refine your communication for
     professional technical environments. Since your input was a request for code,
     I have framed your intent as a formal inquiry.

     ---

     ### Natural English Version

     "Could you provide a canonical 'Hello World' implementation in Python?
     I am looking to establish a baseline for my environment."

     ### Grammar Correction

     - **Original:** "Write python hello world program"
     - **Correction:** The original lacks an article ("a") and proper
       capitalization ("Python"). In professional English, imperative sentences
       ("Write...") can come across as demanding. Using a polite modal verb like
       "Could" or stating your intent ("I am looking to...") is preferred.
     - **Korean Speaker Note:** Korean often omits articles (a/the). In English
       technical documentation and professional communication, articles are
       essential for clarity. Also, "program" is a countable noun, so it
       requires an article ("a program").

● Write(/tmp/hello_world.py)
  ⎿  Wrote 1 lines to /tmp/hello_world.py
      1 print("Hello, World!")

● Created /tmp/hello_world.py with a simple hello world program. Run it with:

  python /tmp/hello_world.py

  Output:
  Hello, World!
```

## How It Works

```text
You type a prompt
       │
       ▼
[gemini_hook] reads hook JSON from stdin, calls Gemini API
       │
       ▼
Gemini returns proofread/improved text
       │
       ▼
[gemini_hook] outputs { "systemMessage": "..." } to stdout
       │
       ▼
Claude Code injects the system message before Claude responds
```

Prompts starting with `/` (slash commands) are passed through unchanged.

### Claude's context is not affected

The hook outputs a `systemMessage` JSON field:

```json
{ "systemMessage": "### Natural English Version\n..." }
```

This appears as the `⎿ UserPromptSubmit says: ...` annotation you see in the terminal — it is
**only a UI display**, not additional context injected into Claude. Claude receives your original
prompt exactly as typed, with nothing prepended, appended, or modified.

Gemini's feedback is for you, not for Claude.

## Gemini API

- **Model**: `gemini-3.1-flash-lite-preview` (default, configurable via `--model`)
- **Endpoint**: `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`
- **Auth**: `x-goog-api-key` header with `$GEMINI_API_KEY`

Example request:

```sh
# curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite-preview:generateContent" \
curl "https://generativelanguage.googleapis.com/v1beta/models/gemma-4-31b-it:generateContent" \
  -H "x-goog-api-key: $GEMINI_API_KEY" \
  -H "Content-Type: application/json" \
  -X POST \
  -d '{
    "systemInstruction": {
      "parts": [{ "text": "You are a proofreading assistant. Fix grammar and spelling errors." }]
    },
    "contents": [
      { "parts": [{ "text": "Explain how AI works in a few words" }] }
    ]
  }'
```

The useful field in the response is `candidates[0].content.parts[0].text`. The `thoughtSignature`
field is an internal reasoning trace from the model and should be ignored.

## Binary

A single binary `gemini_hook` handles the `UserPromptSubmit` hook event.

### Flags

| Flag                        | Env              | Default                         | Description                                     |
| --------------------------- | ---------------- | ------------------------------- | ----------------------------------------------- |
| `--gemini-api-key`          | `GEMINI_API_KEY` | (required)                      | Gemini API key                                  |
| `--model`                   | —                | `gemini-3.1-flash-lite-preview` | Gemini model to use                             |
| `--system-instruction-file` | —                | (none)                          | Path to a file with a custom system instruction |

## Installation

Use `install.sh` to build, install the binary, and configure Claude Code in one step:

```sh
export GEMINI_API_KEY=your_api_key_here
./install.sh --system-instruction-file /path/to/instruction.txt [--model gemini-3.1-flash-lite-preview]
```

The script:

1. Builds the release binary with `cargo build --release`
2. Copies it to `~/.local/bin/gemini_hook`
3. Updates `~/.claude/settings.json` to register the `UserPromptSubmit` hook

Requires: `cargo`, `jq`

### Resulting hook entry in `settings.json`

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "statusMessage": "Proofreading...",
            "command": "~/.local/bin/gemini_hook --system-instruction-file /path/to/instruction.txt --model gemini-3.1-flash-lite-preview"
          }
        ]
      }
    ]
  }
}
```

## Environment

```sh
export GEMINI_API_KEY=your_api_key_here
```

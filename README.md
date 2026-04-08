# gemini-proofread-hook

A Claude Code hook that proofreads your prompts using Gemini before Claude sees them,
injecting corrections as a system message into Claude's context.

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

## Gemini API

- **Model**: `gemini-3.1-flash-lite-preview` (default, configurable via `--model`)
- **Endpoint**: `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`
- **Auth**: `x-goog-api-key` header with `$GEMINI_API_KEY`

Example request:

```sh
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite-preview:generateContent" \
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

# gemini-proofread-hook

A Claude Code hook that proofreads your prompts using Gemini in the background,
then shows suggestions after Claude responds — without blocking or affecting Claude's context.

## How It Works

```
You type a prompt
       │
       ▼
[submit] parses it, spawns [worker] as detached process, exits immediately
       │
       ▼
Claude responds normally (worker is calling Gemini in parallel)
       │
       ▼
[stop] checks if worker finished, prints suggestion to stderr
```

## Design Rationale

- **Non-blocking**: `submit` exits in milliseconds. Gemini's free tier is slow (~3–10s);
  making the hook wait would freeze Claude Code on every prompt.
- **Detached child process for worker**: a background thread inside `submit` would be
  killed when `submit` exits. A separate binary survives independently.
- **stderr only**: Claude Code injects hook stdout into Claude's context. stderr goes to
  the terminal only — proofread suggestions never influence Claude's responses.
- **Done-flag pattern**: `worker` writes the result file first, then the `.done` flag.
  `stop` only reads after seeing the flag, avoiding partial-read race conditions.
- **Silent skip**: if Gemini isn't done by the time `stop` runs, the result is discarded
  rather than blocking the terminal.

## Gemini API

- **Model**: `gemini-3-flash-preview`
- **Endpoint**: `https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent`
- **Auth**: `x-goog-api-key` header with `$GEMINI_API_KEY`

Example request (see `gemini_test.sh`):

```sh
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent" \
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

Example response (see `response.json`):

```json
{
  "candidates": [
    {
      "content": {
        "parts": [
          {
            "text": "AI processes vast amounts of data to recognize patterns and make predictions.",
            "thoughtSignature": "<internal reasoning trace — ignore this field>"
          }
        ],
        "role": "model"
      },
      "finishReason": "STOP"
    }
  ],
  "modelVersion": "gemini-3-flash-preview"
}
```

The useful field is `candidates[0].content.parts[0].text`. The `thoughtSignature` is an
internal reasoning trace from the model and should be ignored.

## Binaries

| Binary | Hook event | Role |
|--------|-----------|------|
| `submit` | `UserPromptSubmit` | Parse stdin, spawn `worker`, exit 0 immediately |
| `worker` | (none — detached) | Call Gemini API, write result + done flag to `/tmp` |
| `stop` | `Stop` | Check done flag, print result to stderr, clean up |

## Temp Files

| Path | Purpose |
|------|---------|
| `/tmp/proofread-{session_id}` | Gemini result text |
| `/tmp/proofread-{session_id}.done` | Written last; signals result is fully ready |

## Claude Code Settings

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "type": "command",
        "command": "/path/to/submit",
        "timeout": 5
      }
    ],
    "Stop": [
      {
        "type": "command",
        "command": "/path/to/stop",
        "timeout": 5
      }
    ]
  }
}
```

## Environment

```sh
export GEMINI_API_KEY=your_api_key_here
```

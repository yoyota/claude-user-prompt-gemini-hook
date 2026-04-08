# Gemini Proofread Hook Plan

## Overview

Use Claude Code hooks to proofread user prompts via Gemini API in the background,
then display correction suggestions to the user after Claude finishes responding.
All hook binaries are written in Rust. The system is designed to be completely
non-blocking and invisible to Claude — it only surfaces results to the user via stderr.

## Architecture

```
User submits prompt
        │
        ▼
[UserPromptSubmit hook] ── `submit` binary
        │
        ├── Parse stdin JSON, extract prompt + session_id
        ├── Spawn `worker` as a detached child process (passes prompt + session_id as args)
        └── Exit 0 immediately (Claude Code is unblocked)
        │
        ▼
Claude processes and responds normally
(meanwhile, `worker` is calling Gemini API in the background)
        │
        ▼
[Stop hook] ── `stop` binary
        │
        ├── Parse stdin JSON, extract session_id
        ├── Check if /tmp/proofread-{session_id}.done exists
        ├── If exists: read result, print to stderr, delete temp files
        └── If not exists: skip silently
        │
        ▼
User sees proofread suggestion in terminal (stderr only)
```

## Binaries

### `submit` — UserPromptSubmit hook

Responsible for receiving the hook event and immediately handing off work.

- Reads stdin JSON, extracts `prompt` and `session_id`
- Spawns `worker` as a **detached child process** with prompt and session_id as arguments
- Exits with code 0 immediately

**Why a separate binary instead of spawning a thread?**
If `submit` spawned a tokio thread and then exited, the async runtime would be dropped
and the in-flight HTTP request would be cancelled. A detached child process is fully
independent of the parent's lifetime — it continues running even after `submit` exits.

### `worker` — Background Gemini API caller

The actual long-running work lives here, fully decoupled from the hook lifecycle.

- Receives `prompt` and `session_id` via CLI arguments
- Calls Gemini API (may take several seconds on free tier)
- Writes result to `/tmp/proofread-{session_id}`
- Writes a done flag to `/tmp/proofread-{session_id}.done` atomically after result is written

**Why write the result file before the done flag?**
The `stop` binary checks for the `.done` flag as a signal that the result file is fully
written and safe to read. If we only had one file, `stop` could read a partially written
result. The two-file pattern acts as a lightweight write-then-signal protocol.

### `stop` — Stop hook

Responsible for surfacing the result to the user without affecting Claude's context.

- Reads stdin JSON, extracts `session_id`
- Checks for `/tmp/proofread-{session_id}.done` (non-blocking, no wait)
- If found: reads result file, prints to **stderr**, deletes both temp files
- If not found: exits silently (Gemini was too slow — result is discarded)

**Why stderr and not stdout?**
Claude Code hooks capture stdout and can inject it into Claude's context. Writing to
stderr ensures the proofread output is shown in the terminal only and never influences
Claude's understanding of the conversation.

**Why not wait for the worker to finish?**
Stop hook runs after Claude's response, but the hook itself is still blocking — Claude
Code waits for it to exit before returning control to the user. Waiting indefinitely for
a slow Gemini response would freeze the terminal after every turn. Silently skipping is
the better UX tradeoff.

## Temp File Convention

| File | Purpose |
|------|---------|
| `/tmp/proofread-{session_id}` | Gemini result text |
| `/tmp/proofread-{session_id}.done` | Written last; signals result is fully ready |

Using `session_id` in filenames ensures concurrent Claude Code sessions do not collide.

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

The `timeout: 5` on `submit` is a safety net only — the binary should exit in
milliseconds. The `timeout: 5` on `stop` limits how long it can block the terminal
after Claude responds; in practice it should also be near-instant since it does no
waiting.

## Rust Project Structure

```
gemini-proofread-hook/
├── Cargo.toml
└── src/
    ├── bin/
    │   ├── submit.rs   # UserPromptSubmit hook: parse stdin, spawn worker, exit 0
    │   ├── worker.rs   # Detached process: call Gemini API, write result to /tmp
    │   └── stop.rs     # Stop hook: check done flag, print result to stderr
    └── lib.rs          # Shared: Gemini API client, temp file paths, stdin parsing
```

## Gemini API Usage

- Model: `gemini-3-flash-preview` (free tier)
- Task: light grammar and clarity proofread of the user prompt
- Output format: brief suggestion, or "OK" if no issues found

### API Request Format

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
      { "parts": [{ "text": "<user prompt here>" }] }
    ]
  }'
```

### Response Parsing

The result text is at: `candidates[0].content.parts[0].text`

Note: The response also contains a `thoughtSignature` field (internal reasoning trace from
the model's thinking process). This field should be ignored — only `text` is needed.

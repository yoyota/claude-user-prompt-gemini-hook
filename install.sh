#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 --system-instruction-file <path> [--model <model>] [--log-dir <path>]"
  exit 1
}

SYSTEM_INSTRUCTION_FILE=""
MODEL="gemini-3.1-flash-lite-preview"
LOG_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --system-instruction-file) SYSTEM_INSTRUCTION_FILE="$2"; shift 2 ;;
    --model) MODEL="$2"; shift 2 ;;
    --log-dir) LOG_DIR="$2"; shift 2 ;;
    *) usage ;;
  esac
done

[[ -z "$SYSTEM_INSTRUCTION_FILE" ]] && usage
SYSTEM_INSTRUCTION_FILE="$(realpath "$SYSTEM_INSTRUCTION_FILE")"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/bin"
SETTINGS="$HOME/.claude/settings.json"

# 1. Release build
echo "Building release binary..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

# 2. Copy binary
mkdir -p "$INSTALL_DIR"
cp "$SCRIPT_DIR/target/release/gemini_hook" "$INSTALL_DIR/gemini_hook"
echo "Installed: $INSTALL_DIR/gemini_hook"

# 3. Update settings.json hooks
COMMAND="$INSTALL_DIR/gemini_hook --system-instruction-file $SYSTEM_INSTRUCTION_FILE --model $MODEL"
[[ -n "$LOG_DIR" ]] && COMMAND="$COMMAND --log-dir $LOG_DIR"

HOOK_ENTRY=$(jq -n --arg cmd "$COMMAND" \
  '[{"matcher": "", "hooks": [{"type": "command", "statusMessage": "Proofreading...", "command": $cmd}]}]')

if [ -f "$SETTINGS" ]; then
  tmp=$(mktemp)
  jq --argjson hook "$HOOK_ENTRY" '.hooks.UserPromptSubmit = $hook' "$SETTINGS" > "$tmp"
  mv "$tmp" "$SETTINGS"
else
  mkdir -p "$(dirname "$SETTINGS")"
  jq -n --argjson hook "$HOOK_ENTRY" '{hooks: {UserPromptSubmit: $hook}}' > "$SETTINGS"
fi

echo "Updated $SETTINGS with UserPromptSubmit hook"

#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

REPO="$HOME/Documents/GitHub/alpnest"
DATA="$HOME/.local/share/alpnest"
STATE_DIR="$DATA/state"
LOG_DIR="$DATA/logs"

mkdir -p "$STATE_DIR" "$LOG_DIR"

cd "$REPO"

OLD_HASH_FILE="$STATE_DIR/mail_feed.sha256"
NEW_HASH_FILE="$STATE_DIR/mail_feed.new.sha256"
LOG_FILE="$LOG_DIR/mail-sync.log"

{
  echo
  echo "===== $(date) ====="

  python3 scripts/sync_mail_apple.py \
    --target Google:INBOX:gmail \
    --limit 30 \
    --include-body || true

  python3 scripts/sync_mail_apple.py \
    --target Exchange:Inbox:kit \
    --limit 30 \
    --include-body || true

  python3 scripts/summarize_mail_local.py \
    --model qwen3:8b \
    --limit 30 || true

  cargo run --quiet --bin generate_mail_feed

  if [ -f "$DATA/generated/mail_feed.md" ]; then
    shasum -a 256 "$DATA/generated/mail_feed.md" | awk '{print $1}' > "$NEW_HASH_FILE"

    if [ -f "$OLD_HASH_FILE" ] && ! cmp -s "$OLD_HASH_FILE" "$NEW_HASH_FILE"; then
      osascript -e 'display notification "Mail feed updated" with title "Alpnest" subtitle "New or changed attention mail" sound name "Glass"'
    fi

    mv "$NEW_HASH_FILE" "$OLD_HASH_FILE"
  fi
} >> "$LOG_FILE" 2>&1

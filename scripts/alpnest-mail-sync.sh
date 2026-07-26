#!/usr/bin/env bash
# Periodic Alpnest mail sync.
#
# Syncs every enabled account from config/mail/accounts.cfg over IMAP, then
# runs the local summarizer and regenerates the attention feed. Apple Mail
# targets are still supported for accounts whose provider is apple_local.
set -euo pipefail

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH"

REPO="${ALPNEST_REPO:-$HOME/Documents/GitHub/alpnest}"

# Must agree with src/paths.rs and scripts/paths.py.
if [ -n "${ALPNEST_HOME:-}" ]; then
  DATA="$ALPNEST_HOME"
elif [ "$(uname -s)" = "Darwin" ]; then
  DATA="$HOME/Library/Application Support/alpnest"
else
  DATA="${XDG_DATA_HOME:-$HOME/.local/share}/alpnest"
fi

STATE_DIR="$DATA/state"
LOG_DIR="$DATA/logs"
ACCOUNTS_CFG="$DATA/config/mail/accounts.cfg"

mkdir -p "$STATE_DIR" "$LOG_DIR"

cd "$REPO"

OLD_HASH_FILE="$STATE_DIR/mail_feed.sha256"
NEW_HASH_FILE="$STATE_DIR/mail_feed.new.sha256"
LOG_FILE="$LOG_DIR/mail-sync.log"

{
  echo
  echo "===== $(date) ====="

  if [ -f "$ACCOUNTS_CFG" ]; then
    python3 scripts/sync_mail_imap.py --all --bodies || true
  else
    echo "no accounts.cfg at $ACCOUNTS_CFG; skipping IMAP sync"
  fi

  # Local Apple Mail targets, if the user still keeps any.
  if [ -n "${ALPNEST_APPLE_MAIL_TARGETS:-}" ]; then
    for target in $ALPNEST_APPLE_MAIL_TARGETS; do
      python3 scripts/sync_mail_apple.py \
        --target "$target" \
        --limit 30 \
        --include-body || true
    done
  fi

  python3 scripts/summarize_mail_local.py \
    --model qwen3:8b \
    --limit 30 || true

  cargo run --quiet --bin generate_mail_feed || true

  if [ -f "$DATA/generated/mail_feed.md" ]; then
    shasum -a 256 "$DATA/generated/mail_feed.md" | awk '{print $1}' > "$NEW_HASH_FILE"

    if [ -f "$OLD_HASH_FILE" ] && ! cmp -s "$OLD_HASH_FILE" "$NEW_HASH_FILE"; then
      osascript -e 'display notification "Mail feed updated" with title "Alpnest" subtitle "New or changed attention mail" sound name "Glass"'
    fi

    mv "$NEW_HASH_FILE" "$OLD_HASH_FILE"
  fi
} >> "$LOG_FILE" 2>&1

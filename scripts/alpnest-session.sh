#!/usr/bin/env bash
set -euo pipefail

SESSION_NAME="${ALPNEST_SESSION:-alpnest}"
REPO="$HOME/Documents/GitHub/alpnest"
ALPNEST_COMMAND="${ALPNEST_COMMAND:-alpnest}"
LAYOUT_FILE="${TMPDIR:-/tmp}/alpnest-layout-${USER:-user}.kdl"

cat > "$LAYOUT_FILE" <<LAYOUT
layout {
    pane split_direction="vertical" {
        pane size="65%" {
            command "bash"
            args "-lc" "cd '$REPO' && ALPNEST_NO_SESSION=1 '$ALPNEST_COMMAND'"
        }
        pane size="35%" {
            command "zsh"
            args "-l"
        }
    }
}
LAYOUT

if command -v zellij >/dev/null 2>&1; then
  exec zellij --layout "$LAYOUT_FILE"
fi

if command -v tmux >/dev/null 2>&1; then
  if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
    exec tmux attach -t "$SESSION_NAME"
  fi

  tmux new-session -d -s "$SESSION_NAME" -n nest -c "$REPO"
  tmux send-keys -t "$SESSION_NAME":0.0 "cd '$REPO' && ALPNEST_NO_SESSION=1 '$ALPNEST_COMMAND'" C-m
  tmux split-window -h -t "$SESSION_NAME":0 -c "$REPO"
  tmux send-keys -t "$SESSION_NAME":0.1 "zsh -l" C-m
  tmux select-pane -t "$SESSION_NAME":0.0
  exec tmux attach -t "$SESSION_NAME"
fi

echo "Neither zellij nor tmux found. Install one of them first."
exit 1

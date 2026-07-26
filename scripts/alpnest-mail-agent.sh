#!/usr/bin/env bash
# Install / remove the Alpnest mail LaunchAgent.
#
# The agent runs the IMAP sync on a timer, fetching bodies and summarizing with
# the local model. Summarization wakes Ollama only when something needs it and
# unloads the model afterwards, so a one-minute timer does not pin several GB
# of weights in memory.
#
#   ./scripts/alpnest-mail-agent.sh install [interval-seconds]
#   ./scripts/alpnest-mail-agent.sh uninstall
#   ./scripts/alpnest-mail-agent.sh status
#
# `install` writes the plist and prints the launchctl command to run; it does
# not load the agent for you.
set -euo pipefail

LABEL="com.alpblkba.alpnest.mail"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
REPO="${ALPNEST_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

if [ -n "${ALPNEST_HOME:-}" ]; then
  DATA="$ALPNEST_HOME"
elif [ "$(uname -s)" = "Darwin" ]; then
  DATA="$HOME/Library/Application Support/alpnest"
else
  DATA="${XDG_DATA_HOME:-$HOME/.local/share}/alpnest"
fi

LOG_DIR="$DATA/logs"

usage() {
  echo "usage: $0 {install [interval-seconds]|uninstall|status}" >&2
  exit 2
}

case "${1:-}" in
  install)
    INTERVAL="${2:-60}"
    mkdir -p "$(dirname "$PLIST")" "$LOG_DIR"

    cat > "$PLIST" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/bin/python3</string>
        <string>$REPO/scripts/sync_mail_imap.py</string>
        <string>--all</string>
        <string>--bodies</string>
        <string>--summarize</string>
    </array>
    <key>WorkingDirectory</key>
    <string>$REPO</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>ALPNEST_HOME</key>
        <string>$DATA</string>
        <key>ALPNEST_REPO</key>
        <string>$REPO</string>
        <key>PATH</key>
        <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
    <key>StartInterval</key>
    <integer>$INTERVAL</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/mail-agent.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/mail-agent.log</string>
</dict>
</plist>
PLIST_EOF

    echo "wrote $PLIST (every ${INTERVAL}s)"
    echo
    echo "Load it yourself when you are ready:"
    echo "  launchctl bootstrap gui/\$(id -u) \"$PLIST\""
    echo
    echo "Logs: $LOG_DIR/mail-agent.log"
    ;;

  uninstall)
    if launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1; then
      launchctl bootout "gui/$(id -u)/$LABEL" || true
      echo "unloaded $LABEL"
    fi

    rm -f "$PLIST"
    echo "removed $PLIST"
    ;;

  status)
    if launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1; then
      echo "$LABEL is loaded"
      launchctl print "gui/$(id -u)/$LABEL" | grep -E "state|last exit|run interval" || true
    else
      echo "$LABEL is not loaded"
    fi

    [ -f "$PLIST" ] && echo "plist: $PLIST" || echo "plist: not installed"
    ;;

  *)
    usage
    ;;
esac

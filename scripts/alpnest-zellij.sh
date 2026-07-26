#!/usr/bin/env bash
set -euo pipefail

export ALPNEST_HOME="${ALPNEST_HOME:-$HOME/Library/Application Support/alpnest}"

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
zellij_dir="$ALPNEST_HOME/zellij"

mkdir -p "$zellij_dir"

cat > "$zellij_dir/alpnest.kdl" <<KDL
layout {
    default_tab_template {
        children
    }

    tab name="alpnest" focus=true {
        pane command="cargo" focus=true {
            args "run"
            cwd "$repo_dir"
        }
    }
}
KDL

cat > "$zellij_dir/config.kdl" <<'KDL'
simplified_ui true
pane_frames false
default_mode "locked"
show_startup_tips false
show_release_notes false

ui {
    pane_frames {
        rounded_corners false
        hide_session_name true
    }
}
KDL

exec zellij \
  --config "$zellij_dir/config.kdl" \
  --layout "$zellij_dir/alpnest.kdl"

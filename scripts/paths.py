"""Runtime paths shared by the Alpnest helper scripts.

These must agree with `src/paths.rs`. They used to hardcode
`~/.local/share/alpnest` while the Rust app resolved
`~/Library/Application Support/alpnest` on macOS, so the sync helpers wrote
their output into a directory the TUI never read. Both sides now resolve the
same root.

Overrides, highest priority first:
  ALPNEST_HOME       root for contents/ and config/ (matches the Rust app)
  ALPNEST_DATA_HOME  root for the mail store; defaults to ALPNEST_HOME
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def _default_home() -> Path:
    user_home = Path.home()

    if sys.platform == "darwin":
        return user_home / "Library" / "Application Support" / "alpnest"

    xdg_data_home = os.environ.get("XDG_DATA_HOME")
    if xdg_data_home:
        return Path(xdg_data_home) / "alpnest"

    return user_home / ".local" / "share" / "alpnest"


def _env_path(name: str) -> Path | None:
    value = os.environ.get(name)
    return Path(value).expanduser() if value else None


ALPNEST_HOME = _env_path("ALPNEST_HOME") or _default_home()
DATA_HOME = _env_path("ALPNEST_DATA_HOME") or ALPNEST_HOME

CONTENTS_DIR = ALPNEST_HOME / "contents"
CONFIG_HOME = _env_path("ALPNEST_CONFIG_DIR") or (ALPNEST_HOME / "config")
MAIL_CONFIG_DIR = CONFIG_HOME / "mail"
ACCOUNTS_CFG = MAIL_CONFIG_DIR / "accounts.cfg"

RAW_MAIL_DIR = DATA_HOME / "raw" / "mail" / "messages"
STORE_DIR = DATA_HOME / "store"
GENERATED_DIR = DATA_HOME / "generated"
LOG_DIR = DATA_HOME / "logs"

MESSAGES_JSON = STORE_DIR / "messages.json"
EVENTSTREAMS_JSON = STORE_DIR / "eventstreams.json"
TASKS_JSON = STORE_DIR / "tasks.json"

MAIL_MD = GENERATED_DIR / "mail.md"
MAIL_DECOMPOSITION_MD = GENERATED_DIR / "mail_decomposition.md"
MAIL_SYNC_STATE_JSON = STORE_DIR / "mail_sync_state.json"
TODAY_MD = GENERATED_DIR / "today.md"
CALENDAR_MD = GENERATED_DIR / "calendar.md"


def mail_content_dir() -> Path | None:
    """Locate the mail content directory the TUI renders.

    Prefers a content whose hidden `.<name>.cfg` declares `content_type =
    "mail"`, and falls back to a directory whose name contains "mail".
    """
    if not CONTENTS_DIR.is_dir():
        return None

    fallback: Path | None = None

    for candidate in sorted(CONTENTS_DIR.iterdir()):
        if not candidate.is_dir() or candidate.name.startswith("."):
            continue

        for cfg in candidate.glob(".*.cfg"):
            try:
                raw = cfg.read_text(encoding="utf-8")
            except OSError:
                continue

            for line in raw.splitlines():
                key, _, value = line.partition("=")
                if key.strip() == "content_type" and value.strip().strip('"') == "mail":
                    return candidate

        if fallback is None and "mail" in candidate.name.lower():
            fallback = candidate

    return fallback

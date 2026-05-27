#!/usr/bin/env python3
"""Initialize alpnest's local data directories and store files."""

from __future__ import annotations

import json
from pathlib import Path

from paths import (
    CALENDAR_MD,
    DATA_HOME,
    EVENTSTREAMS_JSON,
    GENERATED_DIR,
    LOG_DIR,
    MAIL_MD,
    MESSAGES_JSON,
    RAW_MAIL_DIR,
    STORE_DIR,
    TASKS_JSON,
    TODAY_MD,
)

EMPTY_JSON_FILES = {
    MESSAGES_JSON: [],
    EVENTSTREAMS_JSON: [],
    TASKS_JSON: [],
}

EMPTY_MARKDOWN_FILES = {
    MAIL_MD: "# mail\n\nNo mail events synced yet.\n",
    TODAY_MD: "# today\n\nNo generated plan yet.\n",
    CALENDAR_MD: "# calendar\n\nNo calendar events synced yet.\n",
}

DIRECTORIES = [
    DATA_HOME,
    RAW_MAIL_DIR,
    STORE_DIR,
    GENERATED_DIR,
    LOG_DIR,
]


def ensure_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def write_json_if_missing(path: Path, value: object) -> None:
    if path.exists():
        return

    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def write_text_if_missing(path: Path, value: str) -> None:
    if path.exists():
        return

    path.write_text(value, encoding="utf-8")


def main() -> int:
    for directory in DIRECTORIES:
        ensure_directory(directory)

    for path, value in EMPTY_JSON_FILES.items():
        write_json_if_missing(path, value)

    for path, value in EMPTY_MARKDOWN_FILES.items():
        write_text_if_missing(path, value)

    print(f"initialized alpnest data home: {DATA_HOME}")
    print(f"raw mail directory: {RAW_MAIL_DIR}")
    print(f"store directory: {STORE_DIR}")
    print(f"generated directory: {GENERATED_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

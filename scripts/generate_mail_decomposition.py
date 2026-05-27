#!/usr/bin/env python3
"""generate a structured mail decomposition snapshot.

this is not an LLM integration. it creates a local, readable snapshot that can
later be inspected by a human, a local model, or an external assistant. 
(because I have no money for using openAI API key)(and no, I am not going to try unethical headless automation methods)
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MAIL_DECOMPOSITION_MD, MAIL_SYNC_STATE_JSON, MESSAGES_JSON


def read_json_list(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []

    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return []

    if not isinstance(value, list):
        return []

    return [item for item in value if isinstance(item, dict)]


def read_json_object(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}

    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}

    return value if isinstance(value, dict) else {}


def compact(value: object, fallback: str = "unknown") -> str:
    if value is None:
        return fallback

    text = str(value).strip()
    return text if text else fallback


def message_lookup(messages: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    lookup: dict[str, dict[str, Any]] = {}

    for message in messages:
        message_id = message.get("id")
        if isinstance(message_id, str) and message_id:
            lookup[message_id] = message

    return lookup


def stream_messages(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    message_ids = stream.get("message_ids")
    if not isinstance(message_ids, list):
        return []

    result = []
    for message_id in message_ids:
        if isinstance(message_id, str) and message_id in messages_by_id:
            result.append(messages_by_id[message_id])

    return result


def render_message(message: dict[str, Any]) -> list[str]:
    lines = [
        f"- id: `{compact(message.get('id'))}`",
        f"  account: `{compact(message.get('account'))}`",
        f"  mailbox: `{compact(message.get('mailbox'))}` / `{compact(message.get('mailbox_name'))}`",
        f"  sender: {compact(message.get('sender'))}",
        f"  subject: {compact(message.get('subject'))}",
        f"  received_at: {compact(message.get('received_at'))}",
        f"  body_status: `{compact(message.get('body_status'), 'unknown')}`",
        f"  body_sync_policy: `{compact(message.get('body_sync_policy'), 'unknown')}`",
        f"  payload_hash: `{compact(message.get('payload_hash'))}`",
    ]

    body_path = message.get("body_path")
    if body_path:
        lines.append(f"  body_path: `{body_path}`")

    snippet = compact(message.get("snippet"), "")
    if snippet:
        lines.append(f"  snippet: {snippet}")
    else:
        lines.append("  snippet: not available in metadata-only sync")

    return lines


def render_stream(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> list[str]:
    messages = stream_messages(stream, messages_by_id)

    lines = [
        f"## stream: {compact(stream.get('id'))}",
        "",
        "### stream metadata",
        "",
        f"- account: `{compact(stream.get('account'))}`",
        f"- sender_key: `{compact(stream.get('sender_key'))}`",
        f"- subject_key: `{compact(stream.get('subject_key'))}`",
        f"- latest_at: {compact(stream.get('latest_at'))}",
        f"- status: `{compact(stream.get('status'), 'active')}`",
        f"- category_guess: `{compact(stream.get('category_guess'), 'uncategorized')}`",
        f"- message_count: {len(messages)}",
        f"- summary: {compact(stream.get('summary'), 'No summary yet.')}",
        "",
        "### messages",
        "",
    ]

    if not messages:
        lines.append("- no messages linked")
        lines.append("")
        return lines

    for message in messages:
        lines.extend(render_message(message))

    lines.append("")
    lines.extend(
        [
            "### decomposition placeholders",
            "",
            "- action_required: unknown",
            "- event_type: unknown",
            "- deadline_guess: unknown",
            "- urgency_guess: unknown",
            "- effort_guess: unknown",
            "- value_guess: unknown",
            "",
        ]
    )

    return lines


def render_snapshot(
    messages: list[dict[str, Any]],
    eventstreams: list[dict[str, Any]],
    sync_state: dict[str, Any],
) -> str:
    messages_by_id = message_lookup(messages)

    lines = [
        "# mail decomposition",
        "",
        "This file is generated from local Apple Mail metadata and event streams.",
        "It is intended for human review and later local/LLM-assisted decomposition.",
        "",
        "## sync state",
        "",
        f"- last_run_at: {compact(sync_state.get('last_run_at'), 'never')}",
        f"- last_new_message_count: {compact(sync_state.get('last_new_message_count'), 'unknown')}",
        f"- include_body: {compact(sync_state.get('include_body'), 'unknown')}",
        f"- limit: {compact(sync_state.get('limit'), 'unknown')}",
        "",
        "## overview",
        "",
        f"- messages: {len(messages)}",
        f"- event_streams: {len(eventstreams)}",
        "",
    ]

    if not eventstreams:
        lines.append("No event streams available.")
        lines.append("")
        return "\n".join(lines)

    lines.append("# event streams")
    lines.append("")

    for stream in sorted(eventstreams, key=lambda item: str(item.get("latest_at", "")), reverse=True):
        lines.extend(render_stream(stream, messages_by_id))

    return "\n".join(lines)


def main() -> int:
    messages = read_json_list(MESSAGES_JSON)
    eventstreams = read_json_list(EVENTSTREAMS_JSON)
    sync_state = read_json_object(MAIL_SYNC_STATE_JSON)

    MAIL_DECOMPOSITION_MD.parent.mkdir(parents=True, exist_ok=True)
    MAIL_DECOMPOSITION_MD.write_text(
        render_snapshot(messages, eventstreams, sync_state),
        encoding="utf-8",
    )

    print(f"rendered mail decomposition: {MAIL_DECOMPOSITION_MD}")
    print(f"messages: {len(messages)}")
    print(f"event streams: {len(eventstreams)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

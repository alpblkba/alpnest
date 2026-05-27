#!/usr/bin/env python3
"""render the normalized alpnest mail store into a markdown view."""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MAIL_MD, MESSAGES_JSON


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


def normalize_datetime(value: object) -> str:
    if not isinstance(value, str) or not value.strip():
        return "unknown time"

    raw = value.strip()

    try:
        parsed = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError:
        return raw

    return parsed.strftime("%Y-%m-%d %H:%M")


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


def account_counts(messages: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}

    for message in messages:
        account = compact(message.get("account"), "unknown")
        counts[account] = counts.get(account, 0) + 1

    return counts


def latest_message_for_stream(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> dict[str, Any] | None:
    message_ids = stream.get("message_ids")
    if not isinstance(message_ids, list):
        return None

    for message_id in reversed(message_ids):
        if isinstance(message_id, str) and message_id in messages_by_id:
            return messages_by_id[message_id]

    return None


def render_stream(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> list[str]:
    latest_message = latest_message_for_stream(stream, messages_by_id)

    account = compact(stream.get("account"), "unknown")
    sender = compact(stream.get("sender_key"), "unknown sender")
    subject = compact(stream.get("subject_key"), "unknown subject")
    status = compact(stream.get("status"), "active")
    category = compact(stream.get("category_guess"), "uncategorized")
    summary = compact(stream.get("summary"), "No summary yet.")
    latest_at = normalize_datetime(stream.get("latest_at"))

    lines = [
        f"## [{account}] {sender} / {subject}",
        "",
        f"- account: `{account}`",
        f"- status: `{status}`",
        f"- category: `{category}`",
        f"- latest: {latest_at}",
        f"- summary: {summary}",
    ]

    if latest_message is not None:
        lines.extend(
            [
                f"- latest sender: {compact(latest_message.get('sender'))}",
                f"- latest subject: {compact(latest_message.get('subject'))}",
                f"- mailbox: {compact(latest_message.get('mailbox'))} / {compact(latest_message.get('mailbox_name'))}",
                f"- body status: {compact(latest_message.get('body_status'), 'unknown')}",
                f"- body sync: {compact(latest_message.get('body_sync_policy'), 'unknown')}",
                f"- snippet: {compact(latest_message.get('snippet'), 'No snippet.')}",
            ]
        )

    derived_task_ids = stream.get("derived_task_ids")
    if isinstance(derived_task_ids, list) and derived_task_ids:
        task_list = ", ".join(str(task_id) for task_id in derived_task_ids)
        lines.append(f"- derived tasks: {task_list}")

    lines.append("")
    return lines


def render_unstreamed_messages(messages: list[dict[str, Any]]) -> list[str]:
    if not messages:
        return []

    lines = ["# unstreamed messages", ""]

    for message in messages:
        account = compact(message.get("account"))
        sender = compact(message.get("sender"))
        subject = compact(message.get("subject"))
        received_at = normalize_datetime(message.get("received_at"))
        snippet = compact(message.get("snippet"), "No snippet.")

        lines.extend(
            [
                f"## [{account}] {sender}",
                "",
                f"- subject: {subject}",
                f"- received: {received_at}",
                f"- snippet: {snippet}",
                "",
            ]
        )

    return lines


def render_mail_view(
    messages: list[dict[str, Any]],
    eventstreams: list[dict[str, Any]],
) -> str:
    messages_by_id = message_lookup(messages)
    counts = account_counts(messages)

    lines = [
        "# mail",
        "",
        f"- messages: {len(messages)}",
        f"- event streams: {len(eventstreams)}",
    ]

    if counts:
        lines.append("- accounts:")
        for account, count in sorted(counts.items()):
            lines.append(f"  - {account}: {count}")

    lines.append("")

    if eventstreams:
        lines.extend(["# event streams", ""])
        for stream in sorted(eventstreams, key=lambda item: str(item.get("latest_at", "")), reverse=True):
            lines.extend(render_stream(stream, messages_by_id))
    elif messages:
        lines.extend(render_unstreamed_messages(messages))
    else:
        lines.append("No mail events synced yet.")
        lines.append("")

    return "\n".join(lines)


def main() -> int:
    messages = read_json_list(MESSAGES_JSON)
    eventstreams = read_json_list(EVENTSTREAMS_JSON)

    MAIL_MD.parent.mkdir(parents=True, exist_ok=True)
    MAIL_MD.write_text(render_mail_view(messages, eventstreams), encoding="utf-8")

    print(f"rendered mail view: {MAIL_MD}")
    print(f"messages: {len(messages)}")
    print(f"event streams: {len(eventstreams)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

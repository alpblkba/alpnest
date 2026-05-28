#!/usr/bin/env python3
"""Render human-readable alpnest mail digests.

This script writes a compact account overview for the TUI Mail panel and one
account-specific digest per mail account. The detailed raw-ish snapshot remains
in mail_decomposition.md.
"""

from __future__ import annotations

import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MAIL_MD, MESSAGES_JSON

MAX_SUMMARY_CHARS = 180
MAX_SUBJECT_CHARS = 96
MAX_SENDER_CHARS = 48
MAX_VISIBLE_STREAMS_PER_ACCOUNT = 20
MAX_ACCOUNT_PREVIEW_ITEMS = 5

ACCOUNT_ORDER = ["kit", "gmail", "icloud", "unknown"]


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


def compact(value: object, fallback: str = "unknown") -> str:
    if value is None:
        return fallback

    text = str(value).strip()
    return text if text else fallback


def truncate(text: str, limit: int) -> str:
    text = " ".join(text.split())

    if len(text) <= limit:
        return text

    return text[: limit - 1].rstrip() + "…"


def slugify(value: str, fallback: str = "unknown") -> str:
    text = value.lower().strip()
    text = re.sub(r"[^a-z0-9_-]+", "-", text)
    text = re.sub(r"-+", "-", text).strip("-")
    return text or fallback


def account_display_name(account: str) -> str:
    known = {
        "kit": "KIT",
        "gmail": "Gmail",
        "icloud": "iCloud",
        "unknown": "Unknown",
    }

    return known.get(account, account.upper())


def account_view_path(account: str) -> Path:
    return MAIL_MD.parent / f"mail_{slugify(account)}.md"


def normalize_datetime(value: object) -> str:
    if not isinstance(value, str) or not value.strip():
        return "unknown time"

    raw = value.strip()

    try:
        parsed = datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError:
        return raw

    return parsed.strftime("%d.%m %H:%M")


def message_lookup(messages: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    lookup: dict[str, dict[str, Any]] = {}

    for message in messages:
        message_id = message.get("id")
        if isinstance(message_id, str) and message_id:
            lookup[message_id] = message

    return lookup


def latest_message_for_stream(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> dict[str, Any] | None:
    message_ids = stream.get("message_ids")
    if not isinstance(message_ids, list):
        return None

    messages = []
    for message_id in message_ids:
        if isinstance(message_id, str) and message_id in messages_by_id:
            messages.append(messages_by_id[message_id])

    if not messages:
        return None

    return sorted(
        messages,
        key=lambda item: (
            str(item.get("received_at", "")),
            item.get("body_status") == "fetched",
        ),
        reverse=True,
    )[0]


def stream_account(stream: dict[str, Any], latest_message: dict[str, Any] | None) -> str:
    account = compact(stream.get("account"), "")
    if account:
        return account

    if latest_message is not None:
        return compact(latest_message.get("account"), "unknown")

    return "unknown"


def stream_sender(stream: dict[str, Any], latest_message: dict[str, Any] | None) -> str:
    sender = compact(stream.get("display_sender"), "")
    if sender:
        return truncate(sender, MAX_SENDER_CHARS)

    if latest_message is not None:
        raw_sender = compact(latest_message.get("sender"), "")
        if raw_sender:
            return truncate(raw_sender, MAX_SENDER_CHARS)

    return truncate(compact(stream.get("sender_key"), "unknown sender"), MAX_SENDER_CHARS)


def stream_subject(stream: dict[str, Any], latest_message: dict[str, Any] | None) -> str:
    subject = compact(stream.get("display_subject"), "")
    if subject:
        return truncate(subject, MAX_SUBJECT_CHARS)

    if latest_message is not None:
        raw_subject = compact(latest_message.get("subject"), "")
        if raw_subject:
            return truncate(raw_subject, MAX_SUBJECT_CHARS)

    return truncate(compact(stream.get("subject_key"), "unknown subject"), MAX_SUBJECT_CHARS)


def stream_summary(stream: dict[str, Any], latest_message: dict[str, Any] | None) -> str:
    summary = compact(stream.get("summary_local"), "")
    if summary:
        return truncate(summary, MAX_SUMMARY_CHARS)

    summary = compact(stream.get("summary"), "")
    if summary and summary != "No summary yet.":
        return truncate(summary, MAX_SUMMARY_CHARS)

    if latest_message is not None:
        snippet = compact(latest_message.get("snippet"), "")
        if snippet:
            return truncate(snippet, MAX_SUMMARY_CHARS)

    if latest_message is not None:
        subject = compact(latest_message.get("subject"), "")
        if subject:
            return f"Metadata-only mail: {truncate(subject, MAX_SUMMARY_CHARS - 20)}"

    return "No readable summary yet."


def stream_category(stream: dict[str, Any]) -> str:
    category = compact(stream.get("category_guess"), "unknown")
    if category == "uncategorized":
        return "unknown"
    return category


def stream_is_noise(stream: dict[str, Any]) -> bool:
    value = stream.get("noise_guess")
    if isinstance(value, bool):
        return value

    category = stream_category(stream)
    return category in {"noise", "newsletter", "social"}


def stream_body_status(latest_message: dict[str, Any] | None) -> str:
    if latest_message is None:
        return "unknown"

    return compact(latest_message.get("body_status"), "unknown")


def stream_message_count(stream: dict[str, Any]) -> int:
    message_ids = stream.get("message_ids")
    if not isinstance(message_ids, list):
        return 0

    return len(message_ids)


def stream_has_unread(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> bool:
    message_ids = stream.get("message_ids")
    if not isinstance(message_ids, list):
        return False

    for message_id in message_ids:
        if not isinstance(message_id, str):
            continue

        message = messages_by_id.get(message_id)
        if message and message.get("unread") is True:
            return True

    return False


def account_stats(
    pairs: list[tuple[dict[str, Any], dict[str, Any] | None]],
    messages_by_id: dict[str, dict[str, Any]],
) -> dict[str, int]:
    return {
        "streams": len(pairs),
        "unread": sum(1 for stream, _ in pairs if stream_has_unread(stream, messages_by_id)),
        "visible": sum(1 for stream, _ in pairs if not stream_is_noise(stream)),
        "noise": sum(1 for stream, _ in pairs if stream_is_noise(stream)),
    }


def sort_key_for_stream(stream: dict[str, Any]) -> str:
    return str(stream.get("latest_at", ""))


def group_streams_by_account(
    eventstreams: list[dict[str, Any]],
    messages_by_id: dict[str, dict[str, Any]],
) -> dict[str, list[tuple[dict[str, Any], dict[str, Any] | None]]]:
    grouped: dict[str, list[tuple[dict[str, Any], dict[str, Any] | None]]] = {}

    for stream in eventstreams:
        latest_message = latest_message_for_stream(stream, messages_by_id)
        account = stream_account(stream, latest_message)
        grouped.setdefault(account, []).append((stream, latest_message))

    for account, streams in grouped.items():
        grouped[account] = sorted(streams, key=lambda pair: sort_key_for_stream(pair[0]), reverse=True)

    return grouped


def ordered_accounts(grouped: dict[str, list[tuple[dict[str, Any], dict[str, Any] | None]]]) -> list[str]:
    known = [account for account in ACCOUNT_ORDER if account in grouped]
    extra = sorted(account for account in grouped if account not in ACCOUNT_ORDER)
    return known + extra


def render_stream_card(stream: dict[str, Any], latest_message: dict[str, Any] | None) -> list[str]:
    sender = stream_sender(stream, latest_message)
    subject = stream_subject(stream, latest_message)
    summary = stream_summary(stream, latest_message)
    category = stream_category(stream)
    latest_at = normalize_datetime(stream.get("latest_at"))
    body_status = stream_body_status(latest_message)
    message_count = stream_message_count(stream)
    source = compact(stream.get("summary_source"), "fallback")

    tags = [category]
    if body_status == "not_fetched":
        tags.append("metadata-only")
    elif body_status == "fetched":
        tags.append("body-fetched")

    if stream_is_noise(stream):
        tags.append("noise")

    return [
        f"## {latest_at} · {sender}",
        f"**{subject}**",
        summary,
        f"tags: {', '.join(tags)} · messages: {message_count} · summary: {source}",
        "",
    ]


def render_noise_section(
    pairs: list[tuple[dict[str, Any], dict[str, Any] | None]],
) -> list[str]:
    noise_pairs = [(stream, msg) for stream, msg in pairs if stream_is_noise(stream)]
    if not noise_pairs:
        return []

    lines = ["## likely noise", ""]

    for stream, latest_message in noise_pairs[:MAX_ACCOUNT_PREVIEW_ITEMS]:
        sender = stream_sender(stream, latest_message)
        subject = stream_subject(stream, latest_message)
        latest_at = normalize_datetime(stream.get("latest_at"))
        category = stream_category(stream)
        lines.append(f"- {latest_at} · {sender}: {subject} ({category})")

    if len(noise_pairs) > MAX_ACCOUNT_PREVIEW_ITEMS:
        lines.append(f"- ... {len(noise_pairs) - MAX_ACCOUNT_PREVIEW_ITEMS} more noise-like streams hidden")

    lines.append("")
    return lines


def render_account_digest(
    account: str,
    pairs: list[tuple[dict[str, Any], dict[str, Any] | None]],
    messages_by_id: dict[str, dict[str, Any]],
) -> str:
    stats = account_stats(pairs, messages_by_id)
    visible_pairs = [(stream, msg) for stream, msg in pairs if not stream_is_noise(stream)]

    lines = [
        f"# {account_display_name(account)} mail",
        "",
        f"{stats['streams']} streams · {stats['unread']} unread · {stats['visible']} visible · {stats['noise']} noise-like",
        f"source: {account_view_path(account)}",
        "",
    ]

    if visible_pairs:
        for stream, latest_message in visible_pairs[:MAX_VISIBLE_STREAMS_PER_ACCOUNT]:
            lines.extend(render_stream_card(stream, latest_message))

        if len(visible_pairs) > MAX_VISIBLE_STREAMS_PER_ACCOUNT:
            hidden_count = len(visible_pairs) - MAX_VISIBLE_STREAMS_PER_ACCOUNT
            lines.append(f"... {hidden_count} more visible streams hidden from this compact digest")
            lines.append("")
    else:
        lines.append("No non-noise mail streams for this account.")
        lines.append("")

    lines.extend(render_noise_section(pairs))
    return "\n".join(lines)


def render_account_preview(
    account: str,
    pairs: list[tuple[dict[str, Any], dict[str, Any] | None]],
    messages_by_id: dict[str, dict[str, Any]],
) -> list[str]:
    stats = account_stats(pairs, messages_by_id)
    visible_pairs = [(stream, msg) for stream, msg in pairs if not stream_is_noise(stream)]
    path = account_view_path(account)

    lines = [
        f"## {account_display_name(account)}",
        f"{stats['streams']} streams · {stats['unread']} unread · {stats['visible']} visible · {stats['noise']} noise-like",
        f"view: {path}",
        "",
    ]

    if not visible_pairs:
        lines.append("No visible mail streams. Noise-like items are hidden from the overview.")
        lines.append("")
        return lines

    for stream, latest_message in visible_pairs[:MAX_ACCOUNT_PREVIEW_ITEMS]:
        sender = stream_sender(stream, latest_message)
        subject = stream_subject(stream, latest_message)
        latest_at = normalize_datetime(stream.get("latest_at"))
        category = stream_category(stream)
        lines.append(f"- {latest_at} · {sender}: {subject} ({category})")

    if len(visible_pairs) > MAX_ACCOUNT_PREVIEW_ITEMS:
        lines.append(f"- ... {len(visible_pairs) - MAX_ACCOUNT_PREVIEW_ITEMS} more visible streams in {path.name}")

    lines.append("")
    return lines


def render_mail_overview(
    messages: list[dict[str, Any]],
    eventstreams: list[dict[str, Any]],
    grouped: dict[str, list[tuple[dict[str, Any], dict[str, Any] | None]]],
    messages_by_id: dict[str, dict[str, Any]],
) -> str:
    noise_count = sum(1 for stream in eventstreams if stream_is_noise(stream))

    lines = [
        "# mail",
        "",
        f"messages: {len(messages)} · event streams: {len(eventstreams)} · noise-like: {noise_count}",
        "",
        "account views are generated separately. in the TUI, this overview can become the mail account selector.",
        "",
    ]

    if eventstreams:
        for account in ordered_accounts(grouped):
            lines.extend(render_account_preview(account, grouped[account], messages_by_id))
    elif messages:
        lines.append("No event streams available yet, but raw messages exist.")
        lines.append("")
    else:
        lines.append("No mail events synced yet.")
        lines.append("")

    return "\n".join(lines)


def remove_stale_account_views(active_accounts: set[str]) -> None:
    for path in MAIL_MD.parent.glob("mail_*.md"):
        account = path.stem.removeprefix("mail_")
        if account not in {slugify(item) for item in active_accounts}:
            path.unlink(missing_ok=True)


def render_mail_files(
    messages: list[dict[str, Any]],
    eventstreams: list[dict[str, Any]],
) -> list[Path]:
    messages_by_id = message_lookup(messages)
    grouped = group_streams_by_account(eventstreams, messages_by_id)
    accounts = ordered_accounts(grouped)
    written = []

    MAIL_MD.parent.mkdir(parents=True, exist_ok=True)
    MAIL_MD.write_text(render_mail_overview(messages, eventstreams, grouped, messages_by_id), encoding="utf-8")
    written.append(MAIL_MD)

    for account in accounts:
        path = account_view_path(account)
        path.write_text(render_account_digest(account, grouped[account], messages_by_id), encoding="utf-8")
        written.append(path)

    remove_stale_account_views(set(accounts))
    return written


def main() -> int:
    messages = read_json_list(MESSAGES_JSON)
    eventstreams = read_json_list(EVENTSTREAMS_JSON)
    written = render_mail_files(messages, eventstreams)

    print(f"rendered mail overview: {MAIL_MD}")
    for path in written:
        if path != MAIL_MD:
            print(f"rendered account view: {path}")
    print(f"messages: {len(messages)}")
    print(f"event streams: {len(eventstreams)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Summarize and lightly classify local mail event streams.

This script is intentionally optional. It uses Ollama when available and falls
back to deterministic summaries when Ollama is unavailable, slow, or returns
invalid JSON.

Input:
  ~/.local/share/alpnest/store/messages.json
  ~/.local/share/alpnest/store/eventstreams.json

Output:
  updated eventstreams.json with display/summary/category/noise fields
"""

from __future__ import annotations

import argparse
import configparser
import json
import re
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MESSAGES_JSON

MAIL_FILTERS_CFG = Path(__file__).with_name("mail_filters.cfg")

DEFAULT_MODEL = "qwen3:8b"
DEFAULT_LIMIT = 10
OLLAMA_TIMEOUT_SECONDS = 90
MAX_BODY_CHARS_FOR_PROMPT = 2500

VALID_CATEGORIES = {
    "school",
    "admin",
    "meeting",
    "assignment",
    "exam",
    "lab",
    "seminar",
    "research",
    "opportunity",
    "newsletter",
    "social",
    "noise",
    "unknown",
}

DEFAULT_IGNORE_SENDER_PATTERNS = [
    "facebookmail.com",
    "slack.com",
    "event.st.com",
]

DEFAULT_IGNORE_SUBJECT_PATTERNS = [
    "newsletter",
    "webinar",
    "onboarding tasks",
    "friend update",
]

DEFAULT_SCHOOL_HINT_PATTERNS = [
    "kit-ilias",
    "ilias",
    "studium.kit.edu",
    "kit.edu",
    "assignment",
    "homework",
    "lecture",
    "lab",
    "exam",
    "seminar",
    "praktikum",
]


def now_iso() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


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


def write_json_list(path: Path, value: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def parse_multiline_patterns(value: str) -> list[str]:
    patterns = []

    for line in value.splitlines():
        pattern = line.strip()
        if not pattern or pattern.startswith("#"):
            continue
        patterns.append(pattern)

    return patterns


def read_pattern_section(
    config: configparser.ConfigParser,
    section: str,
    defaults: list[str],
) -> list[str]:
    if not config.has_section(section):
        return defaults

    raw_patterns = config.get(section, "patterns", fallback="")
    patterns = parse_multiline_patterns(raw_patterns)
    return patterns if patterns else defaults


def read_mail_filters(path: Path = MAIL_FILTERS_CFG) -> dict[str, list[str]]:
    config = configparser.ConfigParser()

    if path.exists():
        config.read(path)

    return {
        "ignore_senders": read_pattern_section(config, "ignore_senders", DEFAULT_IGNORE_SENDER_PATTERNS),
        "ignore_subjects": read_pattern_section(config, "ignore_subjects", DEFAULT_IGNORE_SUBJECT_PATTERNS),
        "school_hints": read_pattern_section(config, "school_hints", DEFAULT_SCHOOL_HINT_PATTERNS),
    }


def compact(value: object, fallback: str = "") -> str:
    if value is None:
        return fallback

    text = str(value).strip()
    return text if text else fallback


def normalize_space(value: str) -> str:
    return " ".join(value.split())


def strip_email_address(sender: str) -> str:
    sender = sender.strip()

    # Common forms: "Name <mail@example.com>" or plain address.
    match = re.match(r"^(.*?)\s*<[^>]+>$", sender)
    if match:
        name = match.group(1).strip().strip('"')
        if name:
            return name

    if "@" in sender:
        local = sender.split("@", 1)[0]
        local = re.sub(r"[-_.+]+", " ", local)
        return local.strip().title() or sender

    return sender


def clean_subject(subject: str) -> str:
    text = subject.strip()
    text = re.sub(r"^\s*(re|fw|fwd|aw|wg)\s*:\s*", "", text, flags=re.IGNORECASE)
    text = re.sub(r"^\[KIT-ILIAS\]\s*", "", text, flags=re.IGNORECASE)
    text = re.sub(r"\s+", " ", text)
    return text.strip() or subject.strip()


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


def latest_message(
    stream: dict[str, Any],
    messages_by_id: dict[str, dict[str, Any]],
) -> dict[str, Any] | None:
    messages = stream_messages(stream, messages_by_id)
    if not messages:
        return None

    return sorted(messages, key=lambda item: str(item.get("received_at", "")), reverse=True)[0]


def read_body_excerpt(message: dict[str, Any]) -> str:
    body_path = message.get("body_path")
    if not isinstance(body_path, str) or not body_path:
        return ""

    path = Path(body_path).expanduser()
    if not path.exists():
        return ""

    try:
        body = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""

    return normalize_space(body)[:MAX_BODY_CHARS_FOR_PROMPT]


def deterministic_category(
    sender: str,
    subject: str,
    body: str,
    filters: dict[str, list[str]],
) -> tuple[str, bool]:
    haystack = f"{sender}\n{subject}\n{body}".lower()

    for pattern in filters["ignore_senders"] + filters["ignore_subjects"]:
        if pattern.lower() in haystack:
            return "noise", True

    if any(pattern.lower() in haystack for pattern in filters["school_hints"]):
        if "exam" in haystack or "prüfung" in haystack:
            return "exam", False
        if "assignment" in haystack or "homework" in haystack or "exercise" in haystack:
            return "assignment", False
        if "lab" in haystack or "praktikum" in haystack:
            return "lab", False
        if "seminar" in haystack:
            return "seminar", False
        return "school", False

    if "meeting" in haystack or "termin" in haystack:
        return "meeting", False

    if "hackathon" in haystack or "internship" in haystack or "application" in haystack:
        return "opportunity", False

    if "newsletter" in haystack or "webinar" in haystack:
        return "newsletter", True

    return "unknown", False


def fallback_summary(message: dict[str, Any], filters: dict[str, list[str]]) -> dict[str, Any]:
    sender = compact(message.get("sender"), "unknown sender")
    subject = compact(message.get("subject"), "unknown subject")
    snippet = compact(message.get("snippet"), "")
    body = read_body_excerpt(message)

    display_sender = strip_email_address(sender)
    display_subject = clean_subject(subject)

    if snippet:
        summary = normalize_space(snippet)
    elif body:
        summary = normalize_space(body)[:260]
    else:
        summary = f"Metadata-only mail: {display_subject}"

    category, noise = deterministic_category(sender, subject, snippet or body, filters)

    return {
        "display_sender": display_sender,
        "display_subject": display_subject,
        "summary": summary,
        "category": category,
        "noise": noise,
    }


def build_prompt(message: dict[str, Any]) -> str:
    sender = compact(message.get("sender"), "unknown sender")
    subject = compact(message.get("subject"), "unknown subject")
    received_at = compact(message.get("received_at"), "unknown time")
    account = compact(message.get("account"), "unknown account")
    snippet = compact(message.get("snippet"), "")
    body = read_body_excerpt(message)
    body_status = compact(message.get("body_status"), "unknown")

    payload = body or snippet or "metadata only; body not fetched"

    return f"""
You are a local mail cleanup helper for a terminal productivity dashboard.
Return compact JSON only. Do not use markdown. Do not add commentary.

Task:
- Make the sender and subject human-readable.
- Summarize the mail in one short sentence.
- Classify it into exactly one category.
- Mark obvious newsletters/social/onboarding/promotional mail as noise.

Allowed categories:
school, admin, meeting, assignment, exam, lab, seminar, research, opportunity, newsletter, social, noise, unknown

Mail metadata:
account: {account}
received_at: {received_at}
sender: {sender}
subject: {subject}
body_status: {body_status}

Mail payload:
{payload}

Return exactly this JSON shape:
{{
  "display_sender": "...",
  "display_subject": "...",
  "summary": "...",
  "category": "school|admin|meeting|assignment|exam|lab|seminar|research|opportunity|newsletter|social|noise|unknown",
  "noise": true
}}
""".strip()


def extract_json_object(value: str) -> dict[str, Any] | None:
    text = value.strip()

    try:
        parsed = json.loads(text)
        if isinstance(parsed, dict):
            return parsed
    except json.JSONDecodeError:
        pass

    start = text.find("{")
    end = text.rfind("}")
    if start == -1 or end == -1 or end <= start:
        return None

    try:
        parsed = json.loads(text[start : end + 1])
    except json.JSONDecodeError:
        return None

    return parsed if isinstance(parsed, dict) else None


def ollama_generate(model: str, prompt: str) -> str:
    result = subprocess.run(
        ["ollama", "run", model, prompt],
        check=True,
        capture_output=True,
        text=True,
        timeout=OLLAMA_TIMEOUT_SECONDS,
    )

    return result.stdout.strip()


def normalize_model_result(raw: dict[str, Any], fallback: dict[str, Any]) -> dict[str, Any]:
    display_sender = compact(raw.get("display_sender"), fallback["display_sender"])
    display_subject = compact(raw.get("display_subject"), fallback["display_subject"])
    summary = compact(raw.get("summary"), fallback["summary"])
    category = compact(raw.get("category"), fallback["category"]).lower()

    if category not in VALID_CATEGORIES:
        category = fallback["category"]

    noise_value = raw.get("noise", fallback["noise"])
    if isinstance(noise_value, bool):
        noise = noise_value
    elif isinstance(noise_value, str):
        noise = noise_value.strip().lower() in {"true", "yes", "1"}
    else:
        noise = bool(fallback["noise"])

    return {
        "display_sender": display_sender[:120],
        "display_subject": display_subject[:180],
        "summary": summary[:500],
        "category": category,
        "noise": noise,
    }


def summarize_message(
    message: dict[str, Any],
    model: str,
    use_ollama: bool,
    filters: dict[str, list[str]],
) -> tuple[dict[str, Any], str]:
    fallback = fallback_summary(message, filters)

    if not use_ollama:
        return fallback, "fallback"

    try:
        output = ollama_generate(model, build_prompt(message))
    except (subprocess.SubprocessError, OSError):
        return fallback, "fallback"

    parsed = extract_json_object(output)
    if parsed is None:
        return fallback, "fallback"

    return normalize_model_result(parsed, fallback), f"ollama:{model}"


def should_resummarize(stream: dict[str, Any], force: bool) -> bool:
    if force:
        return True

    return not stream.get("summary_local")


def summarize_streams(
    messages: list[dict[str, Any]],
    eventstreams: list[dict[str, Any]],
    model: str,
    limit: int,
    use_ollama: bool,
    force: bool,
    filters: dict[str, list[str]],
) -> tuple[list[dict[str, Any]], int]:
    messages_by_id = message_lookup(messages)
    updated = 0

    for stream in eventstreams:
        if updated >= limit:
            break

        if not should_resummarize(stream, force):
            continue

        message = latest_message(stream, messages_by_id)
        if message is None:
            continue

        summary, source = summarize_message(message, model, use_ollama, filters)

        stream["display_sender"] = summary["display_sender"]
        stream["display_subject"] = summary["display_subject"]
        stream["summary_local"] = summary["summary"]
        stream["category_guess"] = summary["category"]
        stream["noise_guess"] = summary["noise"]
        stream["summary_source"] = source
        stream["summary_updated_at"] = now_iso()

        updated += 1

    return eventstreams, updated


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Summarize local alpnest mail streams.")
    parser.add_argument("--model", default=DEFAULT_MODEL, help=f"Ollama model name; default: {DEFAULT_MODEL}")
    parser.add_argument("--limit", type=int, default=DEFAULT_LIMIT, help=f"streams to summarize; default: {DEFAULT_LIMIT}")
    parser.add_argument("--no-ollama", action="store_true", help="use deterministic fallback only")
    parser.add_argument("--force", action="store_true", help="resummarize streams that already have summaries")
    parser.add_argument(
        "--filters",
        default=str(MAIL_FILTERS_CFG),
        help=f"mail filter config path; default: {MAIL_FILTERS_CFG}",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    messages = read_json_list(MESSAGES_JSON)
    eventstreams = read_json_list(EVENTSTREAMS_JSON)
    filters = read_mail_filters(Path(args.filters))

    updated_streams, updated_count = summarize_streams(
        messages=messages,
        eventstreams=eventstreams,
        model=args.model,
        limit=max(args.limit, 0),
        use_ollama=not args.no_ollama,
        force=args.force,
        filters=filters,
    )

    write_json_list(EVENTSTREAMS_JSON, updated_streams)

    print(f"summarized streams: {updated_count}")
    print(f"mail filters: {Path(args.filters)}")
    print(f"event streams store: {EVENTSTREAMS_JSON}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
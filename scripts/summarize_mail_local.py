#!/usr/bin/env python3
"""Summarize and lightly classify local mail event streams.

This script uses a prompt pack under prompts/qwen/mail_summarizer/ to shape
qwen3:8b into a narrow local mail summarizer intern.

Input:
  ~/.local/share/alpnest/store/messages.json
  ~/.local/share/alpnest/store/eventstreams.json

Output:
  updated eventstreams.json with display/summary/category/action/review fields
"""

from __future__ import annotations

import argparse
import configparser
import json
import re
import urllib.error
import urllib.request
from datetime import datetime
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MESSAGES_JSON

REPO_ROOT = Path(__file__).resolve().parents[1]
MAIL_FILTERS_CFG = Path(__file__).with_name("mail_filters.cfg")
DEFAULT_PROMPT_DIR = REPO_ROOT / "prompts/qwen/mail_summarizer"

DEFAULT_MODEL = "qwen3:8b"
DEFAULT_LIMIT = 10
OLLAMA_TIMEOUT_SECONDS = 180
MAX_BODY_CHARS_FOR_PROMPT = 4000

PROMPT_FILES = [
    "system.md",
    "context.md",
    "task.md",
    "output_schema.md",
    "rubric.md",
    "examples.md",
    "failure_modes.md",
]

VALID_CATEGORIES = {
    "school",
    "admin",
    "assignment",
    "exam",
    "lab",
    "seminar",
    "research",
    "project",
    "work",
    "career",
    "application",
    "meeting",
    "event",
    "calendar",
    "security",
    "finance",
    "travel",
    "github",
    "tool",
    "newsletter",
    "promotion",
    "shopping",
    "social",
    "noise",
    "unknown",
}

VALID_LANGUAGES = {"tr", "en", "de", "mixed", "unknown"}
VALID_ATTENTIONS = {"overview", "account_only", "hidden"}
VALID_IMPORTANCES = {"high", "medium", "low"}
VALID_RETENTION_HINTS = {"24h", "3d", "7d", "until_deadline", "keep", "hidden"}

CATEGORY_ALIASES = {
    "opportunity": "career",
    "promo": "promotion",
    "marketing": "promotion",
    "uncategorized": "unknown",
    "academic": "school",
}

OLLAMA_SUMMARY_SCHEMA = {
    "type": "object",
    "properties": {
        "display_sender": {"type": "string"},
        "display_subject": {"type": "string"},
        "summary": {"type": "string"},
        "category": {
            "type": "string",
            "enum": [
                "school",
                "admin",
                "assignment",
                "exam",
                "lab",
                "seminar",
                "research",
                "project",
                "work",
                "career",
                "application",
                "meeting",
                "event",
                "calendar",
                "security",
                "finance",
                "travel",
                "github",
                "tool",
                "newsletter",
                "promotion",
                "shopping",
                "social",
                "noise",
                "unknown",
            ],
        },
        "attention": {
            "type": "string",
            "enum": ["overview", "account_only", "hidden"],
        },
        "importance": {
            "type": "string",
            "enum": ["high", "medium", "low"],
        },
        "action_required": {"type": "boolean"},
        "action": {"type": ["string", "null"]},
        "deadline": {"type": ["string", "null"]},
        "date_or_time": {"type": ["string", "null"]},
        "retention_hint": {
            "type": "string",
            "enum": ["24h", "3d", "7d", "until_deadline", "keep", "hidden"],
        },
        "source_language": {
            "type": "string",
            "enum": ["tr", "en", "de", "mixed", "unknown"],
        },
        "summary_language": {"type": "string", "enum": ["en"]},
        "language": {
            "type": "string",
            "enum": ["tr", "en", "de", "mixed", "unknown"],
        },
        "noise": {"type": "boolean"},
        "needs_human_review": {"type": "boolean"},
        "confidence": {"type": "number"},
    },
    "required": [
        "display_sender",
        "display_subject",
        "summary",
        "category",
        "attention",
        "importance",
        "action_required",
        "action",
        "deadline",
        "date_or_time",
        "retention_hint",
        "source_language",
        "summary_language",
        "noise",
        "needs_human_review",
        "confidence",
    ],
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
    if path.exists():
        text = path.read_text(encoding="utf-8")
        flat_filters = read_flat_mail_filters(text)
        if flat_filters is not None:
            return flat_filters

    config = configparser.ConfigParser()

    if path.exists():
        config.read(path)

    return {
        "ignore_senders": read_pattern_section(config, "ignore_senders", DEFAULT_IGNORE_SENDER_PATTERNS),
        "ignore_subjects": read_pattern_section(config, "ignore_subjects", DEFAULT_IGNORE_SUBJECT_PATTERNS),
        "school_hints": read_pattern_section(config, "school_hints", DEFAULT_SCHOOL_HINT_PATTERNS),
    }


def read_flat_mail_filters(text: str) -> dict[str, list[str]] | None:
    ignore_senders: list[str] = []
    ignore_subjects: list[str] = []
    saw_flat_rule = False

    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        key = key.strip().lower()
        value = value.strip()
        if not value:
            continue

        if key == "sender_contains":
            ignore_senders.append(value)
            saw_flat_rule = True
        elif key == "subject_contains":
            ignore_subjects.append(value)
            saw_flat_rule = True
        elif key in {"body_contains", "summary_contains", "account", "account_contains"}:
            saw_flat_rule = True

    if not saw_flat_rule:
        return None

    return {
        "ignore_senders": ignore_senders or DEFAULT_IGNORE_SENDER_PATTERNS,
        "ignore_subjects": ignore_subjects or DEFAULT_IGNORE_SUBJECT_PATTERNS,
        "school_hints": DEFAULT_SCHOOL_HINT_PATTERNS,
    }


def read_prompt_pack(prompt_dir: Path) -> str:
    sections = []

    for name in PROMPT_FILES:
        path = prompt_dir / name
        if not path.exists():
            continue

        sections.append(f"# {name}\n\n{path.read_text(encoding='utf-8').strip()}")

    if not sections:
        raise FileNotFoundError(f"no prompt files found in {prompt_dir}")

    return "\n\n---\n\n".join(sections)


def compact(value: object, fallback: str = "") -> str:
    if value is None:
        return fallback

    text = str(value).strip()
    return text if text else fallback


def normalize_space(value: str) -> str:
    return " ".join(value.split())


def strip_email_address(sender: str) -> str:
    sender = sender.strip()

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
    text = re.sub(r"^\[KIT-Student\]\s*", "", text, flags=re.IGNORECASE)
    text = re.sub(r"^\[computerscience-master\]\s*", "", text, flags=re.IGNORECASE)
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

    return sorted(
        messages,
        key=lambda item: (
            str(item.get("received_at", "")),
            item.get("body_status") == "fetched",
        ),
        reverse=True,
    )[0]


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


def is_obvious_noise(message: dict[str, Any], filters: dict[str, list[str]]) -> bool:
    sender = compact(message.get("sender"), "")
    subject = compact(message.get("subject"), "")
    body_status = compact(message.get("body_status"), "unknown")

    haystack = f"{sender}\n{subject}".lower()

    if body_status == "fetched":
        return False

    for pattern in filters["ignore_senders"] + filters["ignore_subjects"]:
        if pattern.lower() in haystack:
            return True

    obvious_terms = [
        "facebookmail.com",
        "close_friend_updates",
        "newsletter",
        "webinar",
        "unsubscribe",
        "promo",
        "promotion",
        "discount",
        "sale",
        "live stream",
        "onboarding tasks",
    ]

    return any(term in haystack for term in obvious_terms)


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

    if "github" in haystack:
        return "github", False

    if any(term in haystack for term in ["security", "verify", "verification", "password", "2fa", "two-factor"]):
        return "security", False

    if any(term in haystack for term in ["hackathon", "event", "workshop"]):
        return "event", False

    if any(term in haystack for term in ["internship", "application", "student assistant", "career", "recruiting"]):
        return "application", False

    if "newsletter" in haystack or "webinar" in haystack:
        return "newsletter", True

    return "unknown", False


def normalize_category(value: object, fallback: str = "unknown") -> str:
    category = compact(value, fallback).lower()
    category = CATEGORY_ALIASES.get(category, category)
    return category if category in VALID_CATEGORIES else fallback


def normalize_attention(value: object, fallback: str = "account_only") -> str:
    attention = compact(value, fallback).lower()
    return attention if attention in VALID_ATTENTIONS else fallback


def normalize_importance(value: object, fallback: str = "low") -> str:
    importance = compact(value, fallback).lower()
    return importance if importance in VALID_IMPORTANCES else fallback


def normalize_retention_hint(value: object, fallback: str = "24h") -> str:
    retention_hint = compact(value, fallback).lower()
    return retention_hint if retention_hint in VALID_RETENTION_HINTS else fallback


def normalize_language(value: object, fallback: str = "unknown") -> str:
    language = compact(value, fallback).lower()
    return language if language in VALID_LANGUAGES else fallback


def infer_attention(
    category: str,
    noise: bool,
    action_required: bool,
    deadline: str | None,
    date_or_time: str | None,
    sender: str,
    subject: str,
    body: str,
) -> str:
    if noise or category in {"noise", "promotion", "shopping", "newsletter", "social"}:
        return "hidden"

    haystack = f"{sender}\n{subject}\n{body}".lower()
    overview_categories = {
        "school",
        "assignment",
        "exam",
        "lab",
        "project",
        "work",
        "research",
        "application",
        "career",
        "meeting",
        "calendar",
        "security",
        "finance",
        "github",
    }
    overview_terms = [
        "kit.edu",
        "ilias",
        "hand-in",
        "task",
        "homework",
        "feedback",
        "hiwi",
        "student assistant",
        "application",
        "interview",
        "verify",
        "security",
        "account",
        "deadline",
        "meeting",
        "appointment",
        "registration",
        "approval",
    ]

    if action_required or deadline or category in overview_categories:
        return "overview"

    if date_or_time and category == "event" and any(
        term in haystack for term in ["kit", "school", "career", "research", "hackathon"]
    ):
        return "overview"

    if any(term in haystack for term in overview_terms):
        return "overview"

    return "account_only"


def infer_importance(
    attention: str,
    category: str,
    action_required: bool,
    deadline: str | None,
    date_or_time: str | None,
    needs_human_review: bool,
) -> str:
    if attention == "hidden":
        return "low"
    if action_required or deadline or category in {"security", "exam"}:
        return "high"
    if date_or_time or needs_human_review or attention == "overview":
        return "medium"
    return "low"


def infer_retention_hint(attention: str, importance: str, deadline: str | None, date_or_time: str | None) -> str:
    if attention == "hidden":
        return "hidden"
    if deadline:
        return "until_deadline"
    if importance == "high":
        return "7d"
    if date_or_time or importance == "medium":
        return "3d"
    return "24h"


def backfill_triage_fields(
    eventstreams: list[dict[str, Any]],
    messages_by_id: dict[str, dict[str, Any]],
    filters: dict[str, list[str]],
) -> None:
    for stream in eventstreams:
        message = latest_message(stream, messages_by_id)
        sender = compact(stream.get("display_sender"), "")
        subject = compact(stream.get("display_subject"), "")
        body = compact(stream.get("summary_local", stream.get("summary")), "")
        category = normalize_category(stream.get("category_guess"))
        noise = bool(stream.get("noise_guess", False))

        if message is not None:
            message_sender = compact(message.get("sender"), sender)
            message_subject = compact(message.get("subject"), subject)
            message_body = compact(message.get("snippet"), "") or read_body_excerpt(message)
            sender = sender or message_sender
            subject = subject or message_subject
            body = body or message_body

            if is_obvious_noise(message, filters):
                category = "noise"
                noise = True
            elif category == "unknown":
                category, inferred_noise = deterministic_category(
                    message_sender,
                    message_subject,
                    message_body,
                    filters,
                )
                category = normalize_category(category)
                noise = noise or inferred_noise

        stream["category_guess"] = normalize_category(category)
        stream["noise_guess"] = noise

        existing_attention = stream.get("attention_guess", stream.get("attention"))
        if existing_attention is None:
            stream["attention_guess"] = "hidden" if noise else "account_only"
        else:
            stream["attention_guess"] = normalize_attention(existing_attention)

        existing_importance = stream.get("importance_guess", stream.get("importance"))
        if existing_importance is None:
            stream["importance_guess"] = "low"
        else:
            stream["importance_guess"] = normalize_importance(existing_importance)

        if "retention_hint" in stream and stream.get("retention_hint") is not None:
            stream["retention_hint"] = normalize_retention_hint(stream.get("retention_hint"))
        else:
            stream["retention_hint"] = "hidden" if noise else "24h"

        if not compact(stream.get("source_language")):
            stream["source_language"] = normalize_language(stream.get("language"))
        else:
            stream["source_language"] = normalize_language(stream.get("source_language"))

        stream["summary_language"] = "en"


def fallback_summary(message: dict[str, Any], filters: dict[str, list[str]]) -> dict[str, Any]:
    sender = compact(message.get("sender"), "unknown sender")
    subject = compact(message.get("subject"), "unknown subject")
    snippet = compact(message.get("snippet"), "")
    body = read_body_excerpt(message)
    body_status = compact(message.get("body_status"), "unknown")

    display_sender = strip_email_address(sender)
    display_subject = clean_subject(subject)

    if snippet:
        summary = normalize_space(snippet)
    elif body:
        summary = normalize_space(body)[:260]
    else:
        summary = f"Metadata-only mail: {display_subject}"

    category, noise = deterministic_category(sender, subject, snippet or body, filters)
    category = normalize_category(category)
    needs_human_review = body_status != "fetched" and category in {"school", "admin", "assignment", "exam", "lab", "seminar"}
    attention = infer_attention(
        category,
        noise,
        False,
        None,
        None,
        sender,
        subject,
        snippet or body,
    )
    importance = infer_importance(attention, category, False, None, None, needs_human_review)
    retention_hint = infer_retention_hint(attention, importance, None, None)

    return {
        "display_sender": display_sender,
        "display_subject": display_subject,
        "summary": summary,
        "category": category,
        "attention": attention,
        "importance": importance,
        "action_required": False,
        "action": None,
        "deadline": None,
        "date_or_time": None,
        "retention_hint": retention_hint,
        "language": "unknown",
        "source_language": "unknown",
        "summary_language": "en",
        "noise": noise,
        "needs_human_review": needs_human_review,
        "confidence": 0.45 if needs_human_review else 0.6,
    }


def build_prompt(message: dict[str, Any], prompt_pack: str) -> str:
    sender = compact(message.get("sender"), "unknown sender")
    subject = compact(message.get("subject"), "unknown subject")
    received_at = compact(message.get("received_at"), "unknown time")
    account = compact(message.get("account"), "unknown account")
    snippet = compact(message.get("snippet"), "")
    body = read_body_excerpt(message)
    body_status = compact(message.get("body_status"), "unknown")

    payload = body or snippet or "metadata only; body not fetched"

    return f"""
Use the following prompt contract and examples.

{prompt_pack}

---

Now summarize this mail.

Input:
account: {account}
sender: {sender}
subject: {subject}
received_at: {received_at}
body_status: {body_status}
payload:
{payload}
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
    request_payload = {
        "model": model,
        "prompt": prompt,
        "stream": False,
        "think": False,
        "format": OLLAMA_SUMMARY_SCHEMA,
        "options": {
            "temperature": 0.1,
        },
    }

    request = urllib.request.Request(
        "http://localhost:11434/api/generate",
        data=json.dumps(request_payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    with urllib.request.urlopen(request, timeout=OLLAMA_TIMEOUT_SECONDS) as response:
        response_payload = json.loads(response.read().decode("utf-8"))

    return compact(response_payload.get("response"), "")


def bool_from_model(value: Any, fallback: bool) -> bool:
    if isinstance(value, bool):
        return value

    if isinstance(value, str):
        return value.strip().lower() in {"true", "yes", "1"}

    return fallback


def nullable_string(value: Any) -> str | None:
    if value is None:
        return None

    text = str(value).strip()
    if not text or text.lower() in {"null", "none", "unknown", "n/a"}:
        return None

    return text


def confidence_from_model(value: Any, fallback: float) -> float:
    try:
        confidence = float(value)
    except (TypeError, ValueError):
        return fallback

    return max(0.0, min(confidence, 1.0))


def normalize_model_result(raw: dict[str, Any], fallback: dict[str, Any]) -> dict[str, Any]:
    display_sender = compact(raw.get("display_sender"), fallback["display_sender"])
    display_subject = compact(raw.get("display_subject"), fallback["display_subject"])
    summary = compact(raw.get("summary"), fallback["summary"])
    category = normalize_category(raw.get("category"), fallback["category"])
    action_required = bool_from_model(raw.get("action_required"), fallback["action_required"])
    deadline = nullable_string(raw.get("deadline"))
    date_or_time = nullable_string(raw.get("date_or_time"))
    needs_human_review = bool_from_model(raw.get("needs_human_review"), fallback["needs_human_review"])
    noise = bool_from_model(raw.get("noise"), fallback["noise"])

    source_language = compact(
        raw.get("source_language", raw.get("language")),
        fallback.get("source_language", fallback.get("language", "unknown")),
    ).lower()
    language = source_language

    if source_language not in VALID_LANGUAGES:
        source_language = fallback.get("source_language", fallback["language"])
        language = source_language

    attention = compact(raw.get("attention"), fallback.get("attention", "account_only")).lower()
    if attention not in VALID_ATTENTIONS:
        attention = infer_attention(
            category,
            noise,
            action_required,
            deadline,
            date_or_time,
            display_sender,
            display_subject,
            summary,
        )

    importance = compact(raw.get("importance"), fallback.get("importance", "low")).lower()
    if importance not in VALID_IMPORTANCES:
        importance = infer_importance(
            attention,
            category,
            action_required,
            deadline,
            date_or_time,
            needs_human_review,
        )

    retention_hint = compact(raw.get("retention_hint"), fallback.get("retention_hint", "24h")).lower()
    if retention_hint not in VALID_RETENTION_HINTS:
        retention_hint = infer_retention_hint(attention, importance, deadline, date_or_time)

    return {
        "display_sender": display_sender[:120],
        "display_subject": display_subject[:180],
        "summary": summary[:700],
        "category": category,
        "attention": attention,
        "importance": importance,
        "action_required": action_required,
        "action": nullable_string(raw.get("action")),
        "deadline": deadline,
        "date_or_time": date_or_time,
        "retention_hint": retention_hint,
        "language": language,
        "source_language": source_language,
        "summary_language": "en",
        "noise": noise,
        "needs_human_review": needs_human_review,
        "confidence": confidence_from_model(raw.get("confidence"), fallback["confidence"]),
    }


def summarize_message(
    message: dict[str, Any],
    model: str,
    use_ollama: bool,
    filters: dict[str, list[str]],
    prompt_pack: str,
) -> tuple[dict[str, Any], str]:
    fallback = fallback_summary(message, filters)

    if not use_ollama:
        return fallback, "fallback"

    if is_obvious_noise(message, filters):
        return fallback, "fallback:obvious-noise"

    try:
        output = ollama_generate(model, build_prompt(message, prompt_pack))
    except (urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError):
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
    prompt_pack: str,
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

        sender = compact(message.get("sender"), "unknown sender")
        subject = compact(message.get("subject"), "unknown subject")
        print(
            f"[{updated + 1}/{limit}] summarizing: {sender} / {subject}",
            flush=True,
        )

        summary, source = summarize_message(message, model, use_ollama, filters, prompt_pack)

        print(
            f"[{updated + 1}/{limit}] done: {summary['display_sender']} / {summary['display_subject']} ({source})",
            flush=True,
        )

        stream["display_sender"] = summary["display_sender"]
        stream["display_subject"] = summary["display_subject"]
        stream["summary_local"] = summary["summary"]
        stream["category_guess"] = summary["category"]
        stream["attention"] = summary["attention"]
        stream["attention_guess"] = summary["attention"]
        stream["importance"] = summary["importance"]
        stream["importance_guess"] = summary["importance"]
        stream["action_required"] = summary["action_required"]
        stream["action"] = summary["action"]
        stream["deadline"] = summary["deadline"]
        stream["date_or_time"] = summary["date_or_time"]
        stream["retention_hint"] = summary["retention_hint"]
        stream["language"] = summary["language"]
        stream["source_language"] = summary["source_language"]
        stream["summary_language"] = summary["summary_language"]
        stream["noise_guess"] = summary["noise"]
        stream["needs_human_review"] = summary["needs_human_review"]
        stream["summary_confidence"] = summary["confidence"]
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
    parser.add_argument(
        "--prompt-dir",
        default=str(DEFAULT_PROMPT_DIR),
        help=f"prompt pack directory; default: {DEFAULT_PROMPT_DIR}",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    messages = read_json_list(MESSAGES_JSON)
    eventstreams = read_json_list(EVENTSTREAMS_JSON)
    filters = read_mail_filters(Path(args.filters))
    prompt_pack = read_prompt_pack(Path(args.prompt_dir))

    updated_streams, updated_count = summarize_streams(
        messages=messages,
        eventstreams=eventstreams,
        model=args.model,
        limit=max(args.limit, 0),
        use_ollama=not args.no_ollama,
        force=args.force,
        filters=filters,
        prompt_pack=prompt_pack,
    )
    backfill_triage_fields(updated_streams, message_lookup(messages), filters)

    write_json_list(EVENTSTREAMS_JSON, updated_streams)

    print(f"summarized streams: {updated_count}")
    print(f"mail filters: {Path(args.filters)}")
    print(f"prompt dir: {Path(args.prompt_dir)}")
    print(f"event streams store: {EVENTSTREAMS_JSON}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

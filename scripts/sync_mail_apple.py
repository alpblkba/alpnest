#!/usr/bin/env python3
"""sync recent Apple Mail messages into alpnest's local store.

current model:
  Apple Mail message
      -normalized Message
      -sender-subject EventStream

by default this script captures metadata only. full body fetching is optional
because Apple Mail's `content of message` AppleScript call can be slow on long
or HTML-heavy mail.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Any

from paths import EVENTSTREAMS_JSON, MAIL_SYNC_STATE_JSON, MESSAGES_JSON, RAW_MAIL_DIR

FIELD_SEPARATOR = "\u241f"
RECORD_SEPARATOR = "\u241e"

MAX_BODY_CHARS = 4000
OSASCRIPT_TIMEOUT_SECONDS = 20

DEFAULT_TARGETS = [
    ("Google", "INBOX", "inbox"),
    ("Exchange", "Inbox", "inbox"),
]


def now_iso() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


def slugify(value: str, fallback: str = "unknown") -> str:
    text = value.lower()
    text = re.sub(r"[\[\]\(\)\"'“”‘’<>]", " ", text)
    text = re.sub(r"[^a-z0-9ğüşöçıİĞÜŞÖÇäöüß@._+-]+", "-", text)
    text = text.strip("-")
    text = re.sub(r"-+", "-", text)
    return text if text else fallback


def account_key(account_name: str) -> str:
    name = account_name.lower().strip()

    if "exchange" in name or "kit" in name:
        return "kit"
    if "google" in name or "gmail" in name:
        return "gmail"
    if "icloud" in name:
        return "icloud"

    return slugify(account_name, "unknown-account")


def normalize_subject(subject: str) -> str:
    text = subject.strip()

    while True:
        new_text = re.sub(r"^\s*(re|fw|fwd|aw|wg)\s*:\s*", "", text, flags=re.IGNORECASE)
        if new_text == text:
            break
        text = new_text

    return text.strip()


def short_snippet(body: str, limit: int = 260) -> str:
    text = " ".join(body.split())
    if len(text) <= limit:
        return text
    return text[: limit - 1].rstrip() + "…"


def hash_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8", errors="replace")).hexdigest()[:16]


def stream_id(account: str, sender_key: str, subject_key: str) -> str:
    return f"stream_{account}_{sender_key}_{subject_key}"[:220]


def target_key(account_name: str, mailbox_name: str, mailbox_kind: str) -> str:
    return f"{account_key(account_name)}:{mailbox_kind}:{slugify(mailbox_name)}"


def run_osascript(account_name: str, mailbox_name: str, limit: int, include_body: bool) -> str:
    if include_body:
        body_script = f'''
        set theContent to content of m as string
        set originalLength to length of theContent as string
        set truncatedFlag to "false"

        if length of theContent > {MAX_BODY_CHARS} then
            set theContent to text 1 thru {MAX_BODY_CHARS} of theContent
            set truncatedFlag to "true"
        end if
        '''
    else:
        body_script = '''
        set theContent to ""
        set originalLength to "0"
        set truncatedFlag to "not_fetched"
        '''

    applescript = f'''
tell application "Mail"
    set targetAccount to missing value
    repeat with acc in accounts
        if (name of acc as string) is "{account_name}" then
            set targetAccount to acc
            exit repeat
        end if
    end repeat

    if targetAccount is missing value then
        error "Account not found: {account_name}"
    end if

    set targetMailbox to missing value
    repeat with mb in mailboxes of targetAccount
        if (name of mb as string) is "{mailbox_name}" then
            set targetMailbox to mb
            exit repeat
        end if
    end repeat

    if targetMailbox is missing value then
        error "Mailbox not found: {account_name}/{mailbox_name}"
    end if

    set messageList to messages of targetMailbox
    set messageCount to count of messageList
    set limitCount to {limit}

    if messageCount < limitCount then
        set limitCount to messageCount
    end if

    set outputText to ""

    repeat with i from 1 to limitCount
        set m to item i of messageList

        set theId to id of m as string
        set theSender to sender of m as string
        set theSubject to subject of m as string
        set theDate to date received of m as string
{body_script}
        set outputText to outputText & "{account_name}" & "{FIELD_SEPARATOR}" & "{mailbox_name}" & "{FIELD_SEPARATOR}" & theId & "{FIELD_SEPARATOR}" & theSender & "{FIELD_SEPARATOR}" & theSubject & "{FIELD_SEPARATOR}" & theDate & "{FIELD_SEPARATOR}" & originalLength & "{FIELD_SEPARATOR}" & truncatedFlag & "{FIELD_SEPARATOR}" & theContent & "{RECORD_SEPARATOR}"
    end repeat

    return outputText
end tell
'''

    result = subprocess.run(
        ["osascript", "-e", applescript],
        check=True,
        capture_output=True,
        text=True,
        timeout=OSASCRIPT_TIMEOUT_SECONDS,
    )

    return result.stdout


def parse_records(raw_output: str, mailbox_kind: str) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []

    for raw_record in raw_output.split(RECORD_SEPARATOR):
        raw_record = raw_record.strip()
        if not raw_record:
            continue

        parts = raw_record.split(FIELD_SEPARATOR, 8)
        if len(parts) != 9:
            continue

        account_name, mailbox_name, apple_id, sender, subject, received_at, body_length, body_truncated, body = parts

        account = account_key(account_name)
        normalized_subject = normalize_subject(subject)
        sender_key = slugify(sender, "unknown-sender")
        subject_key = slugify(normalized_subject, "unknown-subject")

        body_was_fetched = body_truncated in {"true", "false"}
        payload_basis = body if body_was_fetched else f"{account_name}|{mailbox_name}|{apple_id}|{sender}|{subject}|{received_at}"
        body_hash = hash_text(payload_basis)

        message_id = f"apple_mail_{account}_{mailbox_kind}_{apple_id}_{body_hash}"
        raw_dir = RAW_MAIL_DIR / account
        raw_path = raw_dir / f"{message_id}.txt"

        record = {
            "id": message_id,
            "source": "apple_mail",
            "account": account,
            "account_name": account_name.strip(),
            "mailbox": mailbox_kind,
            "mailbox_name": mailbox_name.strip(),
            "apple_mail_id": apple_id.strip(),
            "sender": sender.strip(),
            "sender_key": sender_key,
            "subject": subject.strip(),
            "subject_key": subject_key,
            "received_at": received_at.strip(),
            "snippet": short_snippet(body) if body_was_fetched else "",
            "body_path": str(raw_path) if body_was_fetched else None,
            "body_length": int(body_length) if body_length.isdigit() else None,
            "body_truncated": body_truncated == "true",
            "body_status": "fetched" if body_was_fetched else "not_fetched",
            "body_sync_policy": f"first {MAX_BODY_CHARS} characters" if body_truncated == "true" else ("full body" if body_was_fetched else "metadata only"),
            "payload_hash": body_hash,
        }

        records.append(record)

        if body_was_fetched:
            raw_dir.mkdir(parents=True, exist_ok=True)
            if not raw_path.exists():
                raw_path.write_text(body, encoding="utf-8", errors="replace")

    return records


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


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def merge_messages(existing: list[dict[str, Any]], incoming: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], int]:
    merged = {str(item.get("id")): item for item in existing if item.get("id")}
    new_count = 0

    for message in incoming:
        message_id = str(message["id"])
        if message_id not in merged:
            new_count += 1
        merged[message_id] = message

    return (
        sorted(
            merged.values(),
            key=lambda item: str(item.get("received_at", "")),
            reverse=True,
        ),
        new_count,
    )


def update_eventstreams(
    existing_streams: list[dict[str, Any]],
    messages: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    streams = {
        str(stream.get("id")): stream
        for stream in existing_streams
        if stream.get("id")
    }

    for message in messages:
        account = str(message.get("account", "unknown-account"))
        sender_key = str(message.get("sender_key", "unknown-sender"))
        subject_key = str(message.get("subject_key", "unknown-subject"))
        sid = stream_id(account, sender_key, subject_key)

        stream = streams.get(
            sid,
            {
                "id": sid,
                "account": account,
                "sender_key": sender_key,
                "subject_key": subject_key,
                "message_ids": [],
                "latest_at": "",
                "category_guess": "uncategorized",
                "status": "active",
                "summary": "No summary yet.",
                "derived_task_ids": [],
            },
        )

        message_ids = stream.get("message_ids")
        if not isinstance(message_ids, list):
            message_ids = []

        message_id = message.get("id")
        if isinstance(message_id, str) and message_id not in message_ids:
            message_ids.append(message_id)

        stream["message_ids"] = message_ids

        received_at = str(message.get("received_at", ""))
        if received_at >= str(stream.get("latest_at", "")):
            stream["latest_at"] = received_at
            stream["summary"] = message.get("snippet") or "No summary yet."

        streams[sid] = stream

    return sorted(
        streams.values(),
        key=lambda item: str(item.get("latest_at", "")),
        reverse=True,
    )


def update_sync_state(
    targets: list[tuple[str, str, str]],
    incoming_by_target: dict[str, list[dict[str, Any]]],
    new_count: int,
    include_body: bool,
    limit: int,
) -> None:
    state = read_json_object(MAIL_SYNC_STATE_JSON)
    target_state = state.get("targets")
    if not isinstance(target_state, dict):
        target_state = {}

    for account_name, mailbox_name, mailbox_kind in targets:
        key = target_key(account_name, mailbox_name, mailbox_kind)
        messages = incoming_by_target.get(key, [])

        target_state[key] = {
            "account_name": account_name,
            "mailbox_name": mailbox_name,
            "mailbox_kind": mailbox_kind,
            "last_sync_at": now_iso(),
            "last_seen_apple_mail_ids": [message.get("apple_mail_id") for message in messages],
            "last_seen_message_ids": [message.get("id") for message in messages],
            "last_batch_size": len(messages),
            "limit": limit,
            "include_body": include_body,
        }

    state["targets"] = target_state
    state["last_run_at"] = now_iso()
    state["last_new_message_count"] = new_count
    state["include_body"] = include_body
    state["limit"] = limit

    write_json(MAIL_SYNC_STATE_JSON, state)


def sync_target(
    account_name: str,
    mailbox_name: str,
    mailbox_kind: str,
    limit: int,
    include_body: bool,
) -> list[dict[str, Any]]:
    raw_output = run_osascript(account_name, mailbox_name, limit, include_body)
    return parse_records(raw_output, mailbox_kind)


def parse_target(raw: str) -> tuple[str, str, str]:
    parts = raw.split(":", 2)

    if len(parts) == 2:
        account_name, mailbox_name = parts
        mailbox_kind = mailbox_name.lower().replace(" ", "_")
        return account_name, mailbox_name, mailbox_kind

    if len(parts) == 3:
        account_name, mailbox_name, mailbox_kind = parts
        return account_name, mailbox_name, mailbox_kind

    raise ValueError(f"invalid target: {raw}. expected ACCOUNT:MAILBOX or ACCOUNT:MAILBOX:KIND")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Sync recent Apple Mail messages into alpnest.")
    parser.add_argument("--limit", type=int, default=3, help="number of recent messages per target")
    parser.add_argument(
        "--target",
        action="append",
        help="sync target as ACCOUNT:MAILBOX or ACCOUNT:MAILBOX:KIND; can be passed multiple times",
    )
    parser.add_argument(
        "--default-targets",
        action="store_true",
        help="sync default targets: Google/INBOX and Exchange/Inbox",
    )
    parser.add_argument(
        "--include-body",
        action="store_true",
        help="fetch and store message bodies during sync; slower, off by default",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    if args.target:
        targets = [parse_target(item) for item in args.target]
    elif args.default_targets:
        targets = DEFAULT_TARGETS
    else:
        targets = DEFAULT_TARGETS

    print(f"body mode: {'include body' if args.include_body else 'metadata only'}", flush=True)

    incoming_messages: list[dict[str, Any]] = []
    incoming_by_target: dict[str, list[dict[str, Any]]] = {}

    for account_name, mailbox_name, mailbox_kind in targets:
        key = target_key(account_name, mailbox_name, mailbox_kind)
        print(f"syncing {account_name}/{mailbox_name}...", flush=True)

        try:
            messages = sync_target(account_name, mailbox_name, mailbox_kind, args.limit, args.include_body)
            incoming_messages.extend(messages)
            incoming_by_target[key] = messages
            print(f"synced {account_name}/{mailbox_name}: {len(messages)} messages", flush=True)
        except subprocess.TimeoutExpired:
            incoming_by_target[key] = []
            print(
                f"failed to sync {account_name}/{mailbox_name}: osascript timed out after {OSASCRIPT_TIMEOUT_SECONDS}s",
                flush=True,
            )
        except subprocess.CalledProcessError as error:
            incoming_by_target[key] = []
            print(f"failed to sync {account_name}/{mailbox_name}", flush=True)
            print(error.stderr.strip() or error.stdout.strip(), flush=True)
        except Exception as error:
            incoming_by_target[key] = []
            print(f"failed to sync {account_name}/{mailbox_name}: {error}", flush=True)

    existing_messages = read_json_list(MESSAGES_JSON)
    existing_streams = read_json_list(EVENTSTREAMS_JSON)

    merged_messages, new_count = merge_messages(existing_messages, incoming_messages)
    updated_streams = update_eventstreams(existing_streams, merged_messages)

    write_json(MESSAGES_JSON, merged_messages)
    write_json(EVENTSTREAMS_JSON, updated_streams)
    update_sync_state(targets, incoming_by_target, new_count, args.include_body, args.limit)

    print(f"total incoming messages: {len(incoming_messages)}")
    print(f"new messages: {new_count}")
    print(f"messages store: {MESSAGES_JSON}")
    print(f"event streams store: {EVENTSTREAMS_JSON}")
    print(f"sync state: {MAIL_SYNC_STATE_JSON}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

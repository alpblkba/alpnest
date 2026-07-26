#!/usr/bin/env python3
"""Fetch mail for every configured Alpnest account and project it as markdown.

Design notes:

* Read-only. Mailboxes are selected with `readonly=True` and every fetch uses
  `BODY.PEEK`, so syncing never marks a message as read or mutates the server.
* Secrets come from the Keychain at call time (see `mail_accounts.py`) and are
  never logged or written to disk.
* Failures are classified into actionable guidance (see `mail_errors.py`)
  rather than surfacing raw IMAP alert blobs.

Layout written under `$ALPNEST_HOME`, matching content -> panel -> section:

    contents/<mail>/overview.md          combined, low-verbosity feed
    contents/<mail>/<account>/           one panel per account
        overview.md                      summarized message list
        <slug>.md                        one section per mail, full detail
        <slug>.context.md                that mail's summary and triage
    store/messages.json                  merged message records
    store/eventstreams.json              grouped streams the summarizer reads
"""

from __future__ import annotations

import argparse
import email
import email.utils
import hashlib
import imaplib
import json
import os
import re
import ssl
import subprocess
import sys
import time
from datetime import datetime, timezone
from email.header import decode_header, make_header
from pathlib import Path
from typing import Any

import mail_accounts
import mail_errors
from mail_accounts import CredentialError, MailAccount
from paths import (
    EVENTSTREAMS_JSON,
    MAIL_SYNC_STATE_JSON,
    MESSAGES_JSON,
    RAW_MAIL_DIR,
    STORE_DIR,
    mail_content_dir,
)

MAX_BODY_CHARS = 20000
SNIPPET_CHARS = 220
OVERVIEW_LIMIT = 40
ACCOUNT_LIMIT = 200
# History cap per account. The store keeps everything ever fetched; this only
# bounds how many messages are projected as readable sections.
DETAIL_LIMIT = 150

HEADER_FIELDS = "(FROM TO CC REPLY-TO SUBJECT DATE MESSAGE-ID)"

# Only files carrying this marker are ever pruned, so a hand-written section
# inside a mail panel is never deleted by a sync.
GENERATED_MARKER = "<!-- alpnest:generated mail-detail -->"

SCRIPT_DIR = Path(__file__).resolve().parent


def slugify(value: str, fallback: str = "unknown") -> str:
    """Mirrors `slugify` in src/content_editor.rs so ids line up across sides."""
    slug: list[str] = []
    previous_dash = False

    for char in value:
        if char.isascii() and char.isalnum():
            slug.append(char.lower())
            previous_dash = False
        elif not previous_dash:
            slug.append("-")
            previous_dash = True

    result = "".join(slug).strip("-")
    return result or fallback


def decode_mime(value: str | None) -> str:
    if not value:
        return ""

    try:
        return str(make_header(decode_header(value))).strip()
    except (UnicodeDecodeError, LookupError, ValueError):
        return value.strip()


def normalize_subject(subject: str) -> str:
    stripped = re.sub(r"^(?:\s*(?:re|fwd|fw|aw|vs)\s*:\s*)+", "", subject, flags=re.I)
    return stripped.strip() or subject.strip()


def parse_received_at(raw: str | None) -> str:
    if not raw:
        return ""

    try:
        parsed = email.utils.parsedate_to_datetime(raw)
    except (TypeError, ValueError):
        return raw.strip()

    if parsed is None:
        return raw.strip()

    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)

    return parsed.astimezone(timezone.utc).isoformat()


def strip_html(value: str) -> str:
    value = re.sub(r"(?is)<(script|style).*?</\1>", " ", value)
    value = re.sub(r"(?s)<[^>]+>", " ", value)
    value = (
        value.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", '"')
        .replace("&#39;", "'")
    )
    return re.sub(r"[ \t]{2,}", " ", value)


def part_text(part: email.message.Message) -> str:
    payload = part.get_payload(decode=True)

    if payload is None:
        return ""

    charset = part.get_content_charset() or "utf-8"

    try:
        return payload.decode(charset, "replace")
    except (LookupError, UnicodeDecodeError):
        return payload.decode("utf-8", "replace")


def extract_body(message: email.message.Message) -> str:
    """Pull the readable text out of a MIME message.

    `BODY.PEEK[TEXT]` hands back the whole multipart envelope — boundaries,
    per-part headers, base64 blobs — which is unreadable and useless as summary
    input. Walking the parsed tree and taking text/plain (falling back to
    stripped HTML) is what actually yields the message a person wrote.
    """
    if not message.is_multipart():
        text = part_text(message)
        return strip_html(text) if message.get_content_type() == "text/html" else text

    plain: list[str] = []
    html: list[str] = []

    for part in message.walk():
        if part.is_multipart():
            continue

        disposition = str(part.get("Content-Disposition") or "")

        if "attachment" in disposition.lower():
            continue

        content_type = part.get_content_type()

        if content_type == "text/plain":
            plain.append(part_text(part))
        elif content_type == "text/html":
            html.append(part_text(part))

    if plain:
        return "\n".join(plain)

    return strip_html("\n".join(html)) if html else ""


def tidy_body(value: str) -> str:
    """Collapse the runs of blank lines quoted replies leave behind."""
    value = value.replace("\r\n", "\n").replace("\r", "\n")
    value = re.sub(r"\n{3,}", "\n\n", value)
    return value.strip()


def split_addresses(value: str | None) -> list[str]:
    if not value:
        return []

    return [decode_mime(part) for part in value.split(",") if part.strip()]


def connect(account: MailAccount) -> imaplib.IMAP4:
    host = account.resolved_host()
    context = ssl.create_default_context()

    if account.security == "starttls":
        client: imaplib.IMAP4 = imaplib.IMAP4(host, account.imap_port, timeout=30)
        client.starttls(ssl_context=context)
    else:
        client = imaplib.IMAP4_SSL(host, account.imap_port, ssl_context=context, timeout=30)

    client.login(account.login_name(), account.secret())
    return client


LOCK_PATH = STORE_DIR / "sync.lock"
LOCK_STALE_SECONDS = 900


class SyncBusy(RuntimeError):
    pass


def acquire_lock() -> None:
    """Prevent overlapping passes.

    The LaunchAgent fires every 60s while a manual run or a slow summarization
    may still be going. Two passes writing messages.json and eventstreams.json
    at once would interleave and lose records.
    """
    LOCK_PATH.parent.mkdir(parents=True, exist_ok=True)

    try:
        fd = os.open(LOCK_PATH, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError:
        age = time.time() - LOCK_PATH.stat().st_mtime if LOCK_PATH.exists() else 0

        if age < LOCK_STALE_SECONDS:
            raise SyncBusy(
                f"another sync has been running for {int(age)}s; skipping this pass"
            ) from None

        # A previous run died without cleaning up.
        LOCK_PATH.unlink(missing_ok=True)
        fd = os.open(LOCK_PATH, os.O_CREAT | os.O_EXCL | os.O_WRONLY)

    with os.fdopen(fd, "w") as handle:
        handle.write(str(os.getpid()))


def release_lock() -> None:
    LOCK_PATH.unlink(missing_ok=True)


def load_sync_state() -> dict[str, Any]:
    if not MAIL_SYNC_STATE_JSON.exists():
        return {}

    try:
        loaded = json.loads(MAIL_SYNC_STATE_JSON.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}

    return loaded if isinstance(loaded, dict) else {}


def save_sync_state(state: dict[str, Any]) -> None:
    MAIL_SYNC_STATE_JSON.parent.mkdir(parents=True, exist_ok=True)
    MAIL_SYNC_STATE_JSON.write_text(
        json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def fetch_mailbox(
    client: imaplib.IMAP4,
    account: MailAccount,
    mailbox: str,
    initial_limit: int,
    batch_limit: int,
    with_bodies: bool,
    state: dict[str, Any],
) -> list[dict[str, Any]]:
    """Fetch only messages newer than the last sync.

    IMAP UIDs are monotonic within a mailbox as long as UIDVALIDITY is stable,
    so remembering the highest UID seen lets every later pass ask the server
    for `last+1:*` instead of re-downloading the whole mailbox. Already-synced
    mail stays in the store and stays visible.
    """
    status, _ = client.select(f'"{mailbox}"', readonly=True)

    if status != "OK":
        print(f"  ! {account.id}/{mailbox}: mailbox not found on the server", file=sys.stderr)
        return []

    uidvalidity = None
    typ, value = client.response("UIDVALIDITY")

    if typ == "OK" and value and value[0]:
        try:
            uidvalidity = int(value[0])
        except (TypeError, ValueError):
            uidvalidity = None

    key = f"{account.id}/{mailbox}"
    previous = state.get(key) or {}
    last_uid = int(previous.get("last_uid") or 0)

    # A UIDVALIDITY change means the server renumbered the mailbox; every
    # remembered UID is meaningless and we start again.
    if previous.get("uidvalidity") != uidvalidity:
        if last_uid:
            print(f"  · {account.id}/{mailbox}: mailbox renumbered, resyncing")
        last_uid = 0

    if last_uid:
        status, data = client.uid("search", None, f"UID {last_uid + 1}:*")
    else:
        status, data = client.uid("search", None, "ALL")

    if status != "OK" or not data or not data[0]:
        return []

    uids = data[0].split()

    # `UID n:*` always returns at least the highest existing UID, even when
    # nothing is new, so filter explicitly rather than trusting the range.
    if last_uid:
        # Steady state: newest first, capped so one tick cannot flood the
        # summarizer with an unbounded batch.
        uids = [uid for uid in uids if int(uid) > last_uid][-batch_limit:]
    else:
        uids = uids[-initial_limit:]

    if not uids:
        print(f"  · {account.id}/{mailbox}: up to date")
        return []

    records: list[dict[str, Any]] = []

    # With bodies we pull the whole message and parse it, because only the
    # full MIME tree lets us pick out the readable part.
    parts = (
        "BODY.PEEK[] FLAGS"
        if with_bodies
        else f"BODY.PEEK[HEADER.FIELDS {HEADER_FIELDS}] FLAGS"
    )

    highest = last_uid

    for uid in uids:
        status, payload = client.uid("fetch", uid, f"({parts})")

        if status != "OK" or not payload:
            continue

        raw_bytes = b""
        flags = ""

        for item in payload:
            if isinstance(item, tuple):
                marker = item[0].decode("utf-8", "replace") if item[0] else ""

                if "BODY[" in marker or "RFC822" in marker:
                    raw_bytes = item[1]

                flags += marker
            elif isinstance(item, bytes):
                flags += item.decode("utf-8", "replace")

        if not raw_bytes:
            continue

        try:
            highest = max(highest, int(uid))
        except (TypeError, ValueError):
            pass

        parsed = email.message_from_bytes(raw_bytes)
        body_text = tidy_body(extract_body(parsed))[:MAX_BODY_CHARS] if with_bodies else ""
        sender = decode_mime(parsed.get("From"))
        subject = decode_mime(parsed.get("Subject"))
        received_at = parse_received_at(parsed.get("Date"))
        message_id = decode_mime(parsed.get("Message-ID"))
        is_read = "\\Seen" in flags
        flagged = "\\Flagged" in flags
        answered = "\\Answered" in flags

        normalized = normalize_subject(subject)
        basis = message_id or f"{account.id}|{mailbox}|{sender}|{subject}|{received_at}"
        payload_hash = hashlib.sha256(basis.encode("utf-8", "replace")).hexdigest()[:16]
        record_id = f"imap_{account.id}_{slugify(mailbox)}_{payload_hash}"

        body_path = None
        if with_bodies and body_text:
            raw_dir = RAW_MAIL_DIR / account.id
            raw_dir.mkdir(parents=True, exist_ok=True)
            body_file = raw_dir / f"{record_id}.txt"

            # Always rewrite: an improved parser must be able to correct a
            # body cached by an earlier, worse one.
            body_file.write_text(body_text, encoding="utf-8", errors="replace")
            body_path = str(body_file)

        records.append(
            {
                "id": record_id,
                "source": "imap",
                "account": account.id,
                "account_name": account.display_name or account.id,
                "mailbox": slugify(mailbox),
                "mailbox_name": mailbox,
                "sender": sender,
                "sender_key": slugify(sender, "unknown-sender"),
                "subject": subject,
                "subject_key": slugify(normalized, "unknown-subject"),
                "normalized_subject": normalized,
                "received_at": received_at,
                "recipients": split_addresses(parsed.get("To")),
                "cc": split_addresses(parsed.get("Cc")),
                "reply_to": split_addresses(parsed.get("Reply-To")),
                "message_id": message_id,
                "is_read": is_read,
                "unread": not is_read,
                "flagged": flagged,
                "answered": answered,
                "snippet": " ".join(body_text.split())[:SNIPPET_CHARS],
                "body_path": body_path,
                "body_length": len(body_text) if body_text else None,
                "body_truncated": len(body_text) >= MAX_BODY_CHARS if body_text else False,
                "body_status": "fetched" if body_text else "not_fetched",
                "body_sync_policy": (
                    f"first {MAX_BODY_CHARS} characters" if body_text else "metadata only"
                ),
                "payload_hash": payload_hash,
                "uid": int(uid) if uid.isdigit() else None,
            }
        )

    if highest > last_uid:
        state[key] = {"uidvalidity": uidvalidity, "last_uid": highest}

    return records


def sync_account(
    account: MailAccount, with_bodies: bool, state: dict[str, Any]
) -> list[dict[str, Any]]:
    if not account.needs_network:
        print(f"  · {account.id}: local provider, use sync_mail_apple.py instead")
        return []

    account.validate()
    client = connect(account)
    records: list[dict[str, Any]] = []

    try:
        mailbox_count = max(1, len(account.mailboxes))
        initial_per_mailbox = max(1, account.initial_limit // mailbox_count)
        batch_per_mailbox = max(1, account.batch_limit // mailbox_count)

        for mailbox in account.mailboxes:
            fetched = fetch_mailbox(
                client,
                account,
                mailbox,
                initial_per_mailbox,
                batch_per_mailbox,
                with_bodies,
                state,
            )
            records.extend(fetched)

            if fetched:
                print(f"  · {account.id}/{mailbox}: {len(fetched)} new message(s)")
    finally:
        try:
            client.logout()
        except (imaplib.IMAP4.error, OSError):
            pass

    return records


def test_account(account: MailAccount) -> int:
    print(f"testing {account.id} ({account.display_name})", flush=True)

    if not account.needs_network:
        print("  · local provider: nothing to connect to")
        return 0

    try:
        account.validate()
    except ValueError as error:
        print(f"  ! {error}", file=sys.stderr)
        return 1

    print(f"  · host: {account.resolved_host()}:{account.imap_port} ({account.security})", flush=True)
    print(f"  · login as: {account.login_name()}", flush=True)

    try:
        client = connect(account)
    except CredentialError as error:
        print(f"  ! {error}", file=sys.stderr)
        return 1
    except (imaplib.IMAP4.error, OSError, ssl.SSLError) as error:
        diagnosis = mail_errors.classify(
            error, account.id, account.email, account.provider
        )
        print(diagnosis.render(), file=sys.stderr)
        return 1

    try:
        status, mailboxes = client.list()

        if status != "OK":
            print("  ! logged in but could not list mailboxes", file=sys.stderr)
            return 1

        print(f"  · login OK, {len(mailboxes or [])} mailbox(es) visible")

        missing = []
        for mailbox in account.mailboxes:
            status, _ = client.select(f'"{mailbox}"', readonly=True)
            mark = "OK" if status == "OK" else "MISSING"
            print(f"  · {mailbox}: {mark}")

            if status != "OK":
                missing.append(mailbox)

        if missing:
            print(
                "  ! some mailboxes do not exist; adjust the mailboxes field",
                file=sys.stderr,
            )
            return 1
    finally:
        try:
            client.logout()
        except (imaplib.IMAP4.error, OSError):
            pass

    print("  · connection test passed")
    return 0


def read_json_list(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []

    try:
        loaded = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return []

    return [item for item in loaded if isinstance(item, dict)] if isinstance(loaded, list) else []


def write_json_list(path: Path, value: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def merge_messages(new_records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    merged = {
        str(item.get("id")): item for item in read_json_list(MESSAGES_JSON) if item.get("id")
    }

    for record in new_records:
        merged[str(record["id"])] = record

    ordered = newest_first(list(merged.values()))
    write_json_list(MESSAGES_JSON, ordered)
    return ordered


def update_eventstreams(messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Group messages the way the summarizer expects.

    `summarize_mail_local.py` reads `eventstreams.json`, not raw messages, so
    the IMAP sync has to produce streams for the prompt pack to apply at all.
    Grouping key matches sync_mail_apple.py: account + sender + subject.
    """
    streams = {
        str(stream.get("id")): stream
        for stream in read_json_list(EVENTSTREAMS_JSON)
        if stream.get("id")
    }

    for message in messages:
        account = str(message.get("account", "unknown-account"))
        sender_key = str(message.get("sender_key", "unknown-sender"))
        subject_key = str(message.get("subject_key", "unknown-subject"))
        sid = f"{account}:{sender_key}:{subject_key}"

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
            stream.setdefault("display_sender", message.get("sender"))
            stream.setdefault("display_subject", message.get("subject"))

        streams[sid] = stream

    ordered = sorted(streams.values(), key=lambda item: str(item.get("latest_at") or ""), reverse=True)
    write_json_list(EVENTSTREAMS_JSON, ordered)
    return ordered


def run_summarizer(model: str, limit: int) -> None:
    """Delegate to the existing prompt-pack summarizer.

    That script owns the Ollama wake/sleep discipline: it contacts Ollama only
    when something actually needs summarizing and unloads the model afterwards.
    """
    script = SCRIPT_DIR / "summarize_mail_local.py"

    if not script.exists():
        print("  ! summarizer not found; skipping", file=sys.stderr)
        return

    result = subprocess.run(
        [sys.executable, str(script), "--model", model, "--limit", str(limit), "--unload"],
        check=False,
    )

    if result.returncode != 0:
        print("  ! summarizer exited non-zero; continuing with existing summaries", file=sys.stderr)


def summary_index(streams: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    """message id -> the stream that carries its summary."""
    index: dict[str, dict[str, Any]] = {}

    for stream in streams:
        for message_id in stream.get("message_ids", []) or []:
            if isinstance(message_id, str):
                index[message_id] = stream

    return index


def newest_first(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(records, key=lambda item: str(item.get("received_at") or ""), reverse=True)


def summary_for(record: dict[str, Any], summaries: dict[str, dict[str, Any]]) -> str:
    stream = summaries.get(str(record.get("id"))) or {}

    for key in ("summary_local", "summary"):
        value = stream.get(key)
        if isinstance(value, str) and value.strip() and value != "No summary yet.":
            return value.strip()

    snippet = str(record.get("snippet") or "").strip()
    return snippet or "No summary yet."


def detail_slug(record: dict[str, Any]) -> str:
    date_part = str(record.get("received_at") or "")[:10] or "undated"
    subject = slugify(str(record.get("normalized_subject") or record.get("subject") or ""), "no-subject")
    return f"{date_part}-{subject[:48]}-{record.get('payload_hash', 'x')[:8]}"


def render_detail(record: dict[str, Any], summary: str) -> str:
    """The full message, unsummarized — what a mail client would show."""
    body = ""
    body_path = record.get("body_path")

    if body_path:
        try:
            body = Path(body_path).read_text(encoding="utf-8", errors="replace")
        except OSError:
            body = ""

    lines = [
        GENERATED_MARKER,
        f"# {record.get('subject') or '(no subject)'}",
        "",
        f"**From:** {record.get('sender') or 'unknown'}",
    ]

    for label, key in (("To", "recipients"), ("Cc", "cc"), ("Reply-To", "reply_to")):
        values = record.get(key) or []
        if values:
            lines.append(f"**{label}:** {', '.join(values)}")

    received = str(record.get("received_at") or "unknown")
    lines.extend(
        [
            f"**Date:** {received}",
            f"**Mailbox:** {record.get('account')} / {record.get('mailbox_name')}",
            f"**Status:** {'unread' if record.get('unread') else 'read'}"
            + (", flagged" if record.get("flagged") else "")
            + (", answered" if record.get("answered") else ""),
            "",
            "## summary",
            "",
            summary,
            "",
            "## body",
            "",
        ]
    )

    if body:
        lines.append(body.rstrip())

        if record.get("body_truncated"):
            lines.extend(["", f"_(truncated at {MAX_BODY_CHARS} characters)_"])
    else:
        lines.append("_Body was not fetched. Re-run the sync with `--bodies`._")

    return "\n".join(lines) + "\n"


def render_detail_context(record: dict[str, Any], summary: str, stream: dict[str, Any]) -> str:
    lines = [
        GENERATED_MARKER,
        f"# {record.get('sender') or 'unknown'}",
        "",
        summary,
        "",
    ]

    for label, key in (
        ("category", "category_guess"),
        ("attention", "attention"),
        ("importance", "importance"),
        ("action", "action"),
        ("deadline", "deadline"),
    ):
        value = stream.get(key)
        if isinstance(value, str) and value.strip():
            lines.append(f"- {label}: {value.strip()}")

    return "\n".join(lines) + "\n"


def format_row(record: dict[str, Any], summary: str | None = None) -> str:
    mark = "•" if record.get("unread") else " "
    received = str(record.get("received_at") or "")[:16].replace("T", " ")
    sender = record.get("sender") or "unknown sender"
    subject = record.get("subject") or "(no subject)"
    row = f"- {mark} `{received}`  **{sender}** — {subject}"

    if summary and summary != "No summary yet.":
        row += f"\n      {summary}"

    return row


def render_account_overview(
    account: MailAccount,
    records: list[dict[str, Any]],
    summaries: dict[str, dict[str, Any]],
) -> str:
    records = newest_first(records)
    unread = sum(1 for record in records if record.get("unread"))
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    lines = [
        "# All messages",
        "",
        f"{account.display_name or account.id} — {len(records)} message(s), "
        f"{unread} unread, synced {generated}",
        "",
        "_Each message below is also a section on the left; open one to read "
        "the full original._",
        "",
    ]

    if not records:
        lines.append("No messages fetched yet.")
        return "\n".join(lines) + "\n"

    by_mailbox: dict[str, list[dict[str, Any]]] = {}
    for record in records:
        by_mailbox.setdefault(str(record.get("mailbox_name") or "INBOX"), []).append(record)

    for mailbox, mailbox_records in sorted(by_mailbox.items()):
        lines.append(f"## {mailbox}")
        lines.append("")
        for record in mailbox_records[:ACCOUNT_LIMIT]:
            lines.append(format_row(record, summary_for(record, summaries)))
        lines.append("")

    return "\n".join(lines) + "\n"


def render_overview_markdown(
    records: list[dict[str, Any]],
    accounts: list[MailAccount],
    summaries: dict[str, dict[str, Any]],
) -> str:
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    unread = newest_first([record for record in records if record.get("unread")])

    lines = [
        "# mail overview",
        "",
        f"{len(records)} message(s) across {len(accounts)} account(s), "
        f"{len(unread)} unread — synced {generated}",
        "",
    ]

    if accounts:
        lines.extend(["## accounts", ""])
        for account in accounts:
            account_records = [r for r in records if r.get("account") == account.id]
            account_unread = sum(1 for r in account_records if r.get("unread"))
            state = "enabled" if account.enabled else "disabled"
            lines.append(
                f"- **{account.display_name}** (`{account.id}`, {state}) — "
                f"{len(account_records)} message(s), {account_unread} unread"
            )
        lines.append("")

    lines.extend(["## unread, newest first", ""])

    if not unread:
        lines.append("Nothing unread.")
    else:
        for record in unread[:OVERVIEW_LIMIT]:
            lines.append(
                f"{format_row(record, summary_for(record, summaries))}  `{record.get('account')}`"
            )

    lines.append("")
    return "\n".join(lines) + "\n"


def prune_generated(directory: Path, keep: set[str]) -> int:
    """Delete only our own generated detail files that are no longer current."""
    removed = 0

    for path in sorted(directory.glob("*.md")):
        if path.stem in keep:
            continue

        try:
            head = path.read_text(encoding="utf-8", errors="replace")[:200]
        except OSError:
            continue

        if GENERATED_MARKER not in head:
            continue

        try:
            path.unlink()
            removed += 1
        except OSError:
            pass

    return removed


def write_projections(
    records: list[dict[str, Any]],
    accounts: list[MailAccount],
    streams: list[dict[str, Any]],
) -> list[Path]:
    content_dir = mail_content_dir()

    if content_dir is None:
        print(
            "! no mail content found under $ALPNEST_HOME/contents; "
            "create one from the content editor first",
            file=sys.stderr,
        )
        return []

    summaries = summary_index(streams)
    written: list[Path] = []

    overview_path = content_dir / "overview.md"
    overview_path.write_text(
        render_overview_markdown(records, accounts, summaries), encoding="utf-8"
    )
    written.append(overview_path)

    for account in accounts:
        account_records = newest_first(
            [r for r in records if r.get("account") == account.id]
        )
        account_dir = content_dir / slugify(account.id)
        account_dir.mkdir(parents=True, exist_ok=True)

        # `NN-` prefixes drive section order in the registry, so the summary
        # list stays pinned first and messages stay newest-first.
        summary_section = account_dir / "000-overview.md"
        summary_section.write_text(
            render_account_overview(account, account_records, summaries), encoding="utf-8"
        )
        written.append(summary_section)

        keep: set[str] = {"000-overview"}

        detail_cap = DETAIL_LIMIT

        for index, record in enumerate(account_records[:detail_cap], start=1):
            slug = f"{index:03d}-{detail_slug(record)}"
            keep.add(slug)
            keep.add(f"{slug}.context")

            summary = summary_for(record, summaries)
            stream = summaries.get(str(record.get("id"))) or {}

            (account_dir / f"{slug}.md").write_text(
                render_detail(record, summary), encoding="utf-8"
            )
            (account_dir / f"{slug}.context.md").write_text(
                render_detail_context(record, summary, stream), encoding="utf-8"
            )

        pruned = prune_generated(account_dir, keep)

        if pruned:
            print(f"  · {account.id}: pruned {pruned} stale detail section(s)")

        # A flat <account>.md alongside the directory would show up as a second,
        # duplicate panel in the registry.
        legacy = content_dir / f"{slugify(account.id)}.md"
        if legacy.is_file():
            legacy.unlink()

    return written


# Auth failures are not transient: a wrong app password will fail identically
# every time. On a 60s timer that would be ~1400 rejected logins a day, which
# providers treat as an attack. Back off instead until the credential changes.
AUTH_BACKOFF_SECONDS = 1800
AUTH_FAILURE_CODES = {
    "app_password_required",
    "invalid_credentials",
    "auth_failed",
    "microsoft_basic_auth",
}


def auth_backoff_remaining(state: dict[str, Any], account_id: str) -> int:
    entry = state.get(f"auth_failure/{account_id}") or {}
    until = float(entry.get("until") or 0)
    return max(0, int(until - time.time()))


def record_auth_failure(state: dict[str, Any], account_id: str, code: str) -> None:
    state[f"auth_failure/{account_id}"] = {
        "code": code,
        "until": time.time() + AUTH_BACKOFF_SECONDS,
    }


def clear_auth_failure(state: dict[str, Any], account_id: str) -> None:
    state.pop(f"auth_failure/{account_id}", None)


def sync_once(
    selected: list[MailAccount],
    accounts: list[MailAccount],
    with_bodies: bool,
    summarize: bool,
    model: str,
) -> int:
    fetched: list[dict[str, Any]] = []
    failures = 0
    state = load_sync_state()

    for account in selected:
        remaining = auth_backoff_remaining(state, account.id)

        if remaining and len(selected) > 1:
            print(
                f"skipping {account.id}: credential was rejected, retrying in "
                f"{remaining // 60}m (fix it and run --test to clear)",
                flush=True,
            )
            continue

        print(f"syncing {account.id} ({account.display_name})", flush=True)

        try:
            fetched.extend(sync_account(account, with_bodies, state))
            clear_auth_failure(state, account.id)
        except CredentialError as error:
            print(f"  ! {error}", file=sys.stderr)
            record_auth_failure(state, account.id, "missing_secret")
            failures += 1
        except ValueError as error:
            print(f"  ! {error}", file=sys.stderr)
            failures += 1
        except (imaplib.IMAP4.error, OSError, ssl.SSLError) as error:
            diagnosis = mail_errors.classify(
                error, account.id, account.email, account.provider
            )
            print(diagnosis.render(), file=sys.stderr)

            if diagnosis.code in AUTH_FAILURE_CODES:
                record_auth_failure(state, account.id, diagnosis.code)

            failures += 1

    save_sync_state(state)

    # The store is the history: new mail merges in, nothing already fetched is
    # dropped, and the projection below is rebuilt from the whole store.
    merged = merge_messages(fetched) if fetched else read_json_list(MESSAGES_JSON)
    streams = update_eventstreams(merged)

    if fetched:
        print(f"store: {len(merged)} message(s) total, {len(streams)} stream(s)")
    else:
        print(f"no new mail; store holds {len(merged)} message(s)")

    if summarize:
        # Always offer the work: the summarizer itself decides whether
        # anything is pending and only then wakes Ollama. This also picks up
        # streams left unsummarized by an earlier interrupted pass.
        run_summarizer(model, limit=max(20, len(fetched)))
        streams = read_json_list(EVENTSTREAMS_JSON)

    for path in write_projections(merged, accounts, streams):
        print(f"rendered {path}")

    return 1 if failures and not fetched else 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Sync Alpnest mail accounts over IMAP.")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--account", help="sync a single account by id")
    group.add_argument("--all", action="store_true", help="sync every enabled account")
    parser.add_argument(
        "--test",
        action="store_true",
        help="connect and verify credentials without fetching or writing",
    )
    parser.add_argument(
        "--bodies",
        action="store_true",
        help="also fetch message bodies (needed for full-detail sections)",
    )
    parser.add_argument(
        "--summarize",
        action="store_true",
        help="run the local qwen summarizer after fetching",
    )
    parser.add_argument("--model", default="qwen3:8b", help="ollama model for summarization")
    parser.add_argument(
        "--reset",
        action="store_true",
        help="forget UID state so the next pass refetches from scratch",
    )
    parser.add_argument(
        "--watch",
        action="store_true",
        help="keep running, syncing every --interval seconds",
    )
    parser.add_argument(
        "--interval", type=int, default=60, help="seconds between --watch passes"
    )

    args = parser.parse_args()
    accounts = mail_accounts.load()

    if not accounts:
        print(
            "no mail accounts configured; add one from the Configure Mail view (m)",
            file=sys.stderr,
        )
        return 1

    if args.account:
        selected = [account for account in accounts if account.id == args.account]

        if not selected:
            print(f"unknown account: {args.account}", file=sys.stderr)
            return 1
    else:
        selected = [account for account in accounts if account.enabled]

        if not selected:
            print("no enabled accounts to sync", file=sys.stderr)
            return 1

    if args.test:
        # A successful test means the credential is good again, so clear any
        # backoff the timer recorded.
        state = load_sync_state()
        results = []

        for account in selected:
            code = test_account(account)
            results.append(code)

            if code == 0:
                clear_auth_failure(state, account.id)

        save_sync_state(state)
        return max(results)

    STORE_DIR.mkdir(parents=True, exist_ok=True)

    if args.reset:
        state = load_sync_state()
        ids = {account.id for account in selected}
        dropped = [
            key
            for key in list(state)
            if key.split("/", 1)[0] in ids or key.removeprefix("auth_failure/") in ids
        ]

        for key in dropped:
            state.pop(key, None)

        save_sync_state(state)
        print(f"reset sync state for {', '.join(sorted(ids))}")

    if not args.watch:
        try:
            acquire_lock()
        except SyncBusy as busy:
            print(busy, file=sys.stderr)
            return 0

        try:
            return sync_once(selected, accounts, args.bodies, args.summarize, args.model)
        finally:
            release_lock()

    interval = max(15, args.interval)
    print(f"watching {len(selected)} account(s) every {interval}s; ctrl-c to stop")

    while True:
        started = time.monotonic()

        try:
            acquire_lock()

            try:
                sync_once(selected, accounts, args.bodies, args.summarize, args.model)
            finally:
                release_lock()
        except SyncBusy as busy:
            print(busy, file=sys.stderr)
        except KeyboardInterrupt:
            print("stopped")
            return 0
        except Exception as error:  # keep the daemon alive across transient faults
            print(f"! sync pass failed: {error}", file=sys.stderr)

        elapsed = time.monotonic() - started

        try:
            time.sleep(max(1.0, interval - elapsed))
        except KeyboardInterrupt:
            print("stopped")
            return 0


if __name__ == "__main__":
    raise SystemExit(main())

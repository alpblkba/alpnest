"""Turn raw IMAP/SSL failures into guidance the user can act on.

IMAP servers report auth problems as opaque blobs like
`b'[ALERT] Application-specific password required: https://...'`. Showing that
verbatim is what the Configure Mail view used to do; this module maps the
common cases onto a concrete next step instead.
"""

from __future__ import annotations

import re
from dataclasses import dataclass


@dataclass
class Diagnosis:
    """A classified failure: what happened, and what to do about it."""

    code: str
    summary: str
    remedy: list[str]

    def render(self, indent: str = "  ") -> str:
        lines = [f"{indent}! {self.summary}"]
        lines.extend(f"{indent}  {step}" for step in self.remedy)
        return "\n".join(lines)


def _keychain_hint(account_id: str, email: str) -> str:
    label = email or account_id
    return (
        f'security add-generic-password -U -a {account_id} '
        f'-s alpnest-mail -l "alpnest mail: {label}" -w'
    )


def classify(error: BaseException, account_id: str, email: str, provider: str) -> Diagnosis:
    raw = str(error)
    lowered = raw.lower()

    if "application-specific password required" in lowered or "applicationspecific" in lowered:
        return Diagnosis(
            code="app_password_required",
            summary="Gmail rejected the password: this account needs an App Password.",
            remedy=[
                "1. Enable 2-Step Verification: https://myaccount.google.com/signinoptions/two-step-verification",
                "2. Create an App Password: https://myaccount.google.com/apppasswords",
                "3. Store the 16-character password in the Keychain:",
                f"   {_keychain_hint(account_id, email)}",
                "Your normal Google password will never work for IMAP.",
            ],
        )

    if "invalid credentials" in lowered or "authenticationfailed" in lowered:
        return Diagnosis(
            code="invalid_credentials",
            summary="The server rejected the stored credential.",
            remedy=[
                "The Keychain entry exists but the server refused it.",
                "Re-store the correct secret (an App Password for most providers):",
                f"   {_keychain_hint(account_id, email)}",
            ],
        )

    if "authenticate failed" in lowered or "login failed" in lowered:
        if provider == "microsoft":
            return Diagnosis(
                code="microsoft_basic_auth",
                summary="Microsoft refused the login.",
                remedy=[
                    "Microsoft 365 disables basic IMAP auth on most tenants.",
                    "Options, in order of likelihood:",
                    "1. Your tenant (e.g. KIT) may require OAuth2 — basic auth will never succeed.",
                    "2. If app passwords are allowed, create one and store it:",
                    f"   {_keychain_hint(account_id, email)}",
                    "3. Confirm IMAP is enabled for the mailbox at all.",
                    "Check with your IT/RZ before assuming the credential is wrong.",
                ],
            )

        return Diagnosis(
            code="auth_failed",
            summary="The server refused the login.",
            remedy=[
                "Verify the email address and that IMAP is enabled for this account.",
                "Most providers need an app password rather than the account password:",
                f"   {_keychain_hint(account_id, email)}",
            ],
        )

    if "certificate verify failed" in lowered or isinstance(error, __import__("ssl").SSLError):
        return Diagnosis(
            code="tls_failure",
            summary="TLS negotiation failed.",
            remedy=[
                "Check the security mode: SSL/TLS uses port 993, STARTTLS uses 143.",
                "A corporate proxy or VPN can also break certificate validation.",
            ],
        )

    if isinstance(error, OSError) and (
        "nodename nor servname" in lowered
        or "name or service not known" in lowered
        or "temporary failure in name resolution" in lowered
    ):
        return Diagnosis(
            code="dns_failure",
            summary="The IMAP host could not be resolved.",
            remedy=[
                "Check the imap host spelling in the Configure Mail view.",
                "If you are off-campus, some university servers need a VPN.",
            ],
        )

    if isinstance(error, (TimeoutError, ConnectionRefusedError)) or "timed out" in lowered:
        return Diagnosis(
            code="unreachable",
            summary="Could not reach the IMAP server.",
            remedy=[
                "Check network access, the port, and any VPN requirement.",
            ],
        )

    if "does not exist" in lowered or "nonexistent" in lowered:
        return Diagnosis(
            code="mailbox_missing",
            summary="A configured mailbox does not exist on the server.",
            remedy=[
                "Mailbox names are server-specific: Gmail uses INBOX and "
                "[Gmail]/All Mail, Exchange uses Inbox and Archive.",
                "Adjust the mailboxes field in the Configure Mail view.",
            ],
        )

    cleaned = re.sub(r"^b['\"]|['\"]$", "", raw).strip()

    return Diagnosis(
        code="unknown",
        summary=f"Connection failed: {cleaned}",
        remedy=["Run with --test for a step-by-step connection check."],
    )

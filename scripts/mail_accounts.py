"""Read the Alpnest mail account registry and resolve its secrets.

The registry file (`$ALPNEST_HOME/config/mail/accounts.cfg`) is written by the
Configure Mail view and holds connection metadata only. Secrets live in the
macOS Keychain under the service `alpnest-mail`, keyed by account id, and are
read here at sync time — never stored alongside the config.
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass, field
from pathlib import Path

from paths import ACCOUNTS_CFG

KEYCHAIN_SERVICE = "alpnest-mail"

PROVIDER_DEFAULTS: dict[str, tuple[str, int]] = {
    "gmail": ("imap.gmail.com", 993),
    "microsoft": ("outlook.office365.com", 993),
    "icloud": ("imap.mail.me.com", 993),
    "yahoo": ("imap.mail.yahoo.com", 993),
    "imap": ("", 993),
    "apple_local": ("", 0),
}


class CredentialError(RuntimeError):
    """Raised when an account's secret is not available in the Keychain."""


@dataclass
class MailAccount:
    id: str
    display_name: str = ""
    provider: str = "imap"
    email: str = ""
    # IMAP login name when it differs from the address; university servers
    # often authenticate with a short account name. Empty means use the email.
    username: str = ""
    imap_host: str = ""
    imap_port: int = 993
    security: str = "ssl"
    mailboxes: list[str] = field(default_factory=lambda: ["INBOX"])
    # First sync of a mailbox pulls this many; later passes cap at batch_limit.
    initial_limit: int = 20
    batch_limit: int = 10
    enabled: bool = True

    @property
    def needs_network(self) -> bool:
        return self.provider != "apple_local"

    def login_name(self) -> str:
        return self.username.strip() or self.email

    def resolved_host(self) -> str:
        if self.imap_host:
            return self.imap_host
        return PROVIDER_DEFAULTS.get(self.provider, ("", 993))[0]

    def validate(self) -> None:
        if not self.needs_network:
            return

        if not self.email:
            raise ValueError(f"{self.id}: email address is not set")

        if not self.resolved_host():
            raise ValueError(f"{self.id}: imap host is not set")

        if not self.imap_port:
            raise ValueError(f"{self.id}: imap port is not set")

    def secret(self) -> str:
        """Fetch this account's secret from the Keychain.

        The value is returned to the caller for immediate use by the IMAP
        client and is deliberately never written anywhere.
        """
        if not self.needs_network:
            return ""

        try:
            result = subprocess.run(
                [
                    "security",
                    "find-generic-password",
                    "-a",
                    self.id,
                    "-s",
                    KEYCHAIN_SERVICE,
                    "-w",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
        except FileNotFoundError as error:
            raise CredentialError(
                "the `security` command is unavailable; Keychain lookups need macOS"
            ) from error

        if result.returncode != 0 or not result.stdout.strip():
            raise CredentialError(
                f"no Keychain secret for account '{self.id}'. Store one with:\n"
                f"  security add-generic-password -U -a {self.id} "
                f'-s {KEYCHAIN_SERVICE} -l "alpnest mail: {self.email}" -w'
            )

        return result.stdout.rstrip("\n")


def _unquote(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    return value


def parse(raw: str) -> list[MailAccount]:
    blocks: list[tuple[str, dict[str, str]]] = []
    current: tuple[str, dict[str, str]] | None = None

    for line in raw.splitlines():
        line = line.strip()

        if not line or line.startswith("#"):
            continue

        if line.startswith("[") and line.endswith("]"):
            if current is not None:
                blocks.append(current)
                current = None

            header = line[1:-1].strip()
            if header.startswith("account."):
                current = (header[len("account.") :].strip(), {})

            continue

        key, sep, value = line.partition("=")
        if not sep or current is None:
            continue

        current[1][key.strip()] = _unquote(value)

    if current is not None:
        blocks.append(current)

    accounts: list[MailAccount] = []

    for account_id, values in blocks:
        if not account_id:
            continue

        provider = values.get("provider", "imap").strip().lower()
        default_host, default_port = PROVIDER_DEFAULTS.get(provider, ("", 993))

        mailboxes = [
            part.strip()
            for part in values.get("mailboxes", "INBOX").split(",")
            if part.strip()
        ]

        accounts.append(
            MailAccount(
                id=account_id,
                display_name=values.get("display_name", account_id),
                provider=provider,
                email=values.get("email", ""),
                username=values.get("username", ""),
                imap_host=values.get("imap_host", default_host),
                imap_port=int(values.get("imap_port", default_port) or default_port),
                security=values.get("security", "ssl").strip().lower(),
                mailboxes=mailboxes or ["INBOX"],
                initial_limit=int(
                    values.get("initial_limit", values.get("sync_limit", 20)) or 20
                ),
                batch_limit=int(values.get("batch_limit", 10) or 10),
                enabled=values.get("enabled", "true").strip().lower()
                in {"true", "yes", "1"},
            )
        )

    return accounts


def load(path: Path | None = None) -> list[MailAccount]:
    target = path or ACCOUNTS_CFG

    if not target.exists():
        return []

    return parse(target.read_text(encoding="utf-8"))


def find(account_id: str, path: Path | None = None) -> MailAccount | None:
    for account in load(path):
        if account.id == account_id:
            return account

    return None

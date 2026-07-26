"""Pluggable summarization backends with a fixed behavioural contract.

The model is selectable; the *behaviour* is not. Every backend receives the
same prompt pack, is pinned to temperature 0, and is required to return an
object matching the same JSON schema. Everything downstream — category,
attention, importance and retention normalisation, plus the deterministic
fallbacks — runs identically regardless of which backend produced the text.

That is the whole point: swapping qwen for a frontier model may change how well
a summary reads, but it must never change the shape of the data, the allowed
vocabulary, or what Alpnest does with it.

Configuration lives in `[summarizer]` inside
`$ALPNEST_HOME/config/mail/accounts.cfg`. API keys live in the macOS Keychain
under service `alpnest-mail`, account `summarizer:<provider>` — never in the
config file.
"""

from __future__ import annotations

import json
import subprocess
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from paths import ACCOUNTS_CFG

KEYCHAIN_SERVICE = "alpnest-mail"
OLLAMA_URL = "http://localhost:11434"

# Pinned for every backend. Summaries feed a dashboard; run-to-run drift would
# make the same inbox look different on each sync.
TEMPERATURE = 0.0

PROVIDERS = ("ollama", "anthropic", "openai")

DEFAULT_MODELS = {
    "ollama": "qwen3:8b",
    "anthropic": "claude-sonnet-5",
    "openai": "gpt-4o-mini",
}


class BackendError(RuntimeError):
    pass


@dataclass
class SummarizerConfig:
    provider: str = "ollama"
    model: str = DEFAULT_MODELS["ollama"]
    timeout: int = 120

    @property
    def is_local(self) -> bool:
        return self.provider == "ollama"

    def keychain_account(self) -> str:
        return f"summarizer:{self.provider}"

    def api_key(self) -> str:
        if self.is_local:
            return ""

        result = subprocess.run(
            [
                "security",
                "find-generic-password",
                "-a",
                self.keychain_account(),
                "-s",
                KEYCHAIN_SERVICE,
                "-w",
            ],
            capture_output=True,
            text=True,
            check=False,
        )

        if result.returncode != 0 or not result.stdout.strip():
            raise BackendError(
                f"no API key for {self.provider}. Store one with:\n"
                f"  security add-generic-password -U -a {self.keychain_account()} "
                f'-s {KEYCHAIN_SERVICE} -l "alpnest summarizer: {self.provider}" -w'
            )

        return result.stdout.strip()


def load_config(path: Path | None = None) -> SummarizerConfig:
    target = path or ACCOUNTS_CFG
    config = SummarizerConfig()

    if not target.exists():
        return config

    in_block = False

    for line in target.read_text(encoding="utf-8").splitlines():
        line = line.strip()

        if not line or line.startswith("#"):
            continue

        if line.startswith("[") and line.endswith("]"):
            in_block = line[1:-1].strip() == "summarizer"
            continue

        if not in_block:
            continue

        key, sep, value = line.partition("=")

        if not sep:
            continue

        key = key.strip()
        value = value.strip().strip('"')

        if key == "provider" and value in PROVIDERS:
            config.provider = value
        elif key == "model":
            config.model = value
        elif key == "timeout":
            try:
                config.timeout = int(value)
            except ValueError:
                pass

    if not config.model:
        config.model = DEFAULT_MODELS.get(config.provider, "")

    return config


def _post(url: str, payload: dict, headers: dict[str, str], timeout: int) -> dict:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json", **headers},
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", "replace")[:300]
        raise BackendError(f"{url} returned {error.code}: {detail}") from error
    except (urllib.error.URLError, OSError, TimeoutError) as error:
        raise BackendError(f"{url} unreachable: {error}") from error


def _generate_ollama(config: SummarizerConfig, prompt: str, schema: dict) -> str:
    payload = {
        "model": config.model,
        "prompt": prompt,
        "stream": False,
        "think": False,
        "format": schema,
        "options": {"temperature": TEMPERATURE},
    }

    data = _post(f"{OLLAMA_URL}/api/generate", payload, {}, config.timeout)
    return str(data.get("response") or "").strip()


def _generate_anthropic(config: SummarizerConfig, prompt: str, schema: dict) -> str:
    payload = {
        "model": config.model,
        "max_tokens": 1024,
        "temperature": TEMPERATURE,
        "system": (
            "You convert one email into a compact JSON digest. "
            "Reply with a single JSON object matching the given schema and nothing else."
        ),
        "messages": [
            {
                "role": "user",
                "content": f"{prompt}\n\nJSON schema:\n{json.dumps(schema)}",
            }
        ],
    }

    data = _post(
        "https://api.anthropic.com/v1/messages",
        payload,
        {
            "x-api-key": config.api_key(),
            "anthropic-version": "2023-06-01",
        },
        config.timeout,
    )

    parts = [
        block.get("text", "")
        for block in data.get("content", [])
        if block.get("type") == "text"
    ]
    return "".join(parts).strip()


def _generate_openai(config: SummarizerConfig, prompt: str, schema: dict) -> str:
    payload = {
        "model": config.model,
        "temperature": TEMPERATURE,
        "response_format": {"type": "json_object"},
        "messages": [
            {
                "role": "system",
                "content": (
                    "You convert one email into a compact JSON digest. "
                    "Reply with a single JSON object matching the given schema."
                ),
            },
            {
                "role": "user",
                "content": f"{prompt}\n\nJSON schema:\n{json.dumps(schema)}",
            },
        ],
    }

    data = _post(
        "https://api.openai.com/v1/chat/completions",
        payload,
        {"Authorization": f"Bearer {config.api_key()}"},
        config.timeout,
    )

    choices = data.get("choices") or []
    if not choices:
        return ""

    return str(choices[0].get("message", {}).get("content") or "").strip()


def generate(config: SummarizerConfig, prompt: str, schema: dict) -> str:
    """Return raw JSON text from the configured backend."""
    if config.provider == "ollama":
        return _generate_ollama(config, prompt, schema)
    if config.provider == "anthropic":
        return _generate_anthropic(config, prompt, schema)
    if config.provider == "openai":
        return _generate_openai(config, prompt, schema)

    raise BackendError(f"unknown summarizer provider: {config.provider}")


def available(config: SummarizerConfig) -> bool:
    """Cheap reachability probe so an absent backend degrades to fallback."""
    if config.is_local:
        try:
            request = urllib.request.Request(f"{OLLAMA_URL}/api/tags", method="GET")
            with urllib.request.urlopen(request, timeout=3):
                return True
        except (urllib.error.URLError, OSError, TimeoutError):
            return False

    try:
        config.api_key()
        return True
    except BackendError:
        return False


def unload(config: SummarizerConfig) -> bool:
    """Evict a local model from memory. No-op for hosted providers."""
    if not config.is_local:
        return True

    try:
        _post(
            f"{OLLAMA_URL}/api/generate",
            {"model": config.model, "prompt": "", "stream": False, "keep_alive": 0},
            {},
            30,
        )
        return True
    except BackendError:
        return False

#!/usr/bin/env python3
"""Create a reviewable section draft with a local Ollama model."""

from __future__ import annotations

import argparse
import json
import re
import urllib.error
import urllib.request
from pathlib import Path

OLLAMA_GENERATE_URL = "http://127.0.0.1:11434/api/generate"
MAX_BODY_CHARS = 16_000
MAX_CONTEXT_CHARS = 6_000
MAX_PANEL_PROMPT_CHARS = 2_000


def read_optional(path: str | None, limit: int) -> str:
    if not path:
        return "(none)"
    return Path(path).read_text(encoding="utf-8")[:limit]


def build_prompt(body: str, context: str, panel_prompt: str) -> str:
    return f"""You are Alpnest's local section assistant.

Create a concise, reviewable Markdown draft that helps the user move this section forward.
Use only the supplied material. Do not invent facts, requirements, owners, or deadlines.
Prefer 3-7 concrete checkbox steps. Call out missing information explicitly.
Do not rewrite or claim to have changed the source files.
Return Markdown only, beginning with `# Suggested next steps`.

PANEL GUIDANCE
{panel_prompt}

SECTION CONTEXT
{context}

SECTION BODY
{body}
"""


def generate(model: str, prompt: str) -> str:
    payload = json.dumps(
        {
            "model": model,
            "prompt": prompt,
            "stream": False,
            "think": False,
            "keep_alive": "5m",
            "options": {"temperature": 0.1, "num_predict": 600},
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        OLLAMA_GENERATE_URL,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    with urllib.request.urlopen(request, timeout=180) as response:
        result = json.loads(response.read().decode("utf-8"))

    text = str(result.get("response", "")).strip()
    text = re.sub(r"^<think>.*?</think>\s*", "", text, flags=re.DOTALL)

    if not text:
        raise RuntimeError("Ollama returned an empty response")

    return text


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--body", required=True)
    parser.add_argument("--context")
    parser.add_argument("--panel-prompt")
    parser.add_argument("--output", required=True)
    parser.add_argument("--model", default="qwen3:8b")
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    try:
        draft = generate(
            args.model,
            build_prompt(
                read_optional(args.body, MAX_BODY_CHARS),
                read_optional(args.context, MAX_CONTEXT_CHARS),
                read_optional(args.panel_prompt, MAX_PANEL_PROMPT_CHARS),
            ),
        )
    except (OSError, urllib.error.URLError, json.JSONDecodeError, RuntimeError) as error:
        print(f"local model failed: {error}")
        print(f"run `alpnest doctor`; if needed, run `ollama pull {args.model}`")
        return 1

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f"{output.name}.tmp")
    temporary.write_text(
        f"{draft.rstrip()}\n\n> Generated locally with `{args.model}`. Review before applying.\n",
        encoding="utf-8",
    )
    temporary.replace(output)
    print(f"local draft written: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

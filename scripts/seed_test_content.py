#!/usr/bin/env python3
"""Seed (or remove) throwaway School panels for exercising the TUI.

Creates three panels with five sections each, ordered after the real courses
so they never sit between them. Every generated file carries a marker, and
`--remove` deletes only marked files, so a hand-edited file inside a seeded
panel survives cleanup.

    ./scripts/seed_test_content.py --seed
    ./scripts/seed_test_content.py --remove
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from paths import CONTENTS_DIR

MARKER = "<!-- alpnest:generated test-seed -->"

PANELS = [
    ("test-alpha", "Test Alpha", 90),
    ("test-beta", "Test Beta", 91),
    ("test-gamma", "Test Gamma", 92),
]

SECTIONS = [
    (
        "00-overview",
        "Overview",
        "What this panel is for and where it currently stands.",
        [
            ("read the module description", True),
            ("note the assessment format", True),
            ("decide weekly time budget", False),
        ],
    ),
    (
        "10-notes",
        "Notes",
        "Running notes. Probe here for what is still unresolved.",
        [
            ("summarise lecture 1", True),
            ("summarise lecture 2", False),
            ("collect open questions", False),
        ],
    ),
    (
        "20-milestones",
        "Milestones",
        "Ordered milestones. Probe here for progress state.",
        [
            ("ms0: environment ready", True),
            ("ms1: first assignment submitted", False),
            ("ms2: midterm prepared", False),
            ("ms3: project draft", False),
        ],
    ),
    (
        "30-exercises",
        "Exercises",
        "Worked attempts and their results.",
        [
            ("sheet 1", True),
            ("sheet 2", False),
            ("sheet 3", False),
        ],
    ),
    (
        "40-references",
        "References",
        "External sources collected for this panel.",
        [
            ("add the main textbook", False),
            ("bookmark the lecture recordings", False),
        ],
    ),
]


def school_dir() -> Path | None:
    for candidate in ("school", "School"):
        path = CONTENTS_DIR / candidate
        if path.is_dir():
            return path

    return None


def render_body(title: str, steps: list[tuple[str, bool]]) -> str:
    lines = [MARKER, f"# {title}", ""]
    lines.append("Steps below are tracked by the workbench; toggle them with space.")
    lines.append("")

    for text, done in steps:
        lines.append(f"- [{'x' if done else ' '}] {text}")

    lines.extend(["", "## notes", "", "_Replace this with real content._", ""])
    return "\n".join(lines)


def render_context(title: str, blurb: str) -> str:
    return "\n".join([MARKER, f"# {title}", "", blurb, ""])


def seed(root: Path) -> int:
    created = 0

    for slug, title, order in PANELS:
        panel_dir = root / slug
        panel_dir.mkdir(parents=True, exist_ok=True)

        manifest = panel_dir / ".panel.cfg"
        manifest.write_text(
            "schema_version = 1\n"
            f'id = "{slug}"\n'
            f'title = "{title}"\n'
            'kind = "panel"\n'
            "hidden = false\n"
            f"order = {order}\n"
            "prompting_enabled = true\n"
            "deadline_enabled = false\n"
            "deadline_days = 0\n",
            encoding="utf-8",
        )

        (panel_dir / ".prompt.md").write_text(
            f"# {title} panel prompt\n\n"
            "Seeded test panel. Changes here must not affect other panels.\n",
            encoding="utf-8",
        )

        for section_slug, section_title, blurb, steps in SECTIONS:
            (panel_dir / f"{section_slug}.md").write_text(
                render_body(section_title, steps), encoding="utf-8"
            )
            (panel_dir / f"{section_slug}.context.md").write_text(
                render_context(section_title, blurb), encoding="utf-8"
            )
            created += 2

        print(f"seeded {panel_dir}")

    return created


def remove(root: Path) -> int:
    removed = 0

    for slug, _, _ in PANELS:
        panel_dir = root / slug

        if not panel_dir.is_dir():
            continue

        for path in sorted(panel_dir.glob("*.md")):
            # pathlib's glob matches dotfiles; .prompt.md is handled below.
            if path.name.startswith("."):
                continue

            try:
                head = path.read_text(encoding="utf-8", errors="replace")[:200]
            except OSError:
                continue

            if MARKER not in head:
                print(f"kept (edited by hand): {path}")
                continue

            path.unlink()
            removed += 1

        for extra in (".panel.cfg", ".prompt.md"):
            candidate = panel_dir / extra
            if candidate.exists():
                candidate.unlink()

        remaining = list(panel_dir.iterdir())

        if remaining:
            print(f"kept {panel_dir} ({len(remaining)} file(s) left)")
        else:
            panel_dir.rmdir()
            print(f"removed {panel_dir}")

    return removed


def main() -> int:
    parser = argparse.ArgumentParser(description="Seed throwaway School test panels.")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--seed", action="store_true", help="create the test panels")
    group.add_argument("--remove", action="store_true", help="delete generated test files")

    args = parser.parse_args()
    root = school_dir()

    if root is None:
        print(f"no school content under {CONTENTS_DIR}", file=sys.stderr)
        return 1

    if args.seed:
        print(f"seeded {seed(root)} file(s) under {root}")
    else:
        print(f"removed {remove(root)} file(s) under {root}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

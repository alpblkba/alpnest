#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PANELS_ROOT = ROOT / "data" / "panels"


def slugify(value: str) -> str:
    value = value.strip().lower()
    out: list[str] = []
    last_dash = False
    for ch in value:
        if ch.isalnum():
            out.append(ch)
            last_dash = False
        elif not last_dash:
            out.append("-")
            last_dash = True
    return "".join(out).strip("-")


def md(lines: list[str]) -> str:
    return "\n".join(lines).rstrip() + "\n"


def write(path: Path, content: str, *, force: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and not force:
        print(f"kept  {path.relative_to(ROOT)}")
        return
    path.write_text(content, encoding="utf-8")
    print(f"wrote {path.relative_to(ROOT)}")


def parse_toml_value(text: str, key: str) -> str | None:
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        current_key, value = line.split("=", 1)
        if current_key.strip() != key:
            continue
        value = value.strip()
        if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
            return value[1:-1]
        return value
    return None


def find_panel(panel_id: str) -> Path:
    wanted = slugify(panel_id)
    for panel_dir in sorted(PANELS_ROOT.iterdir() if PANELS_ROOT.exists() else []):
        if not panel_dir.is_dir():
            continue
        panel_file = panel_dir / "panel.toml"
        parsed = None
        if panel_file.exists():
            parsed = parse_toml_value(panel_file.read_text(encoding="utf-8"), "id")
        candidates = {slugify(panel_dir.name), slugify(re.sub(r"^\d+-", "", panel_dir.name))}
        if parsed:
            candidates.add(slugify(parsed))
        if wanted in candidates:
            return panel_dir
    raise SystemExit(f"panel not found: {panel_id}")


def next_order_dir(parent: Path, slug: str) -> Path:
    parent.mkdir(parents=True, exist_ok=True)
    max_order = 0
    for child in parent.iterdir():
        if not child.is_dir():
            continue
        match = re.match(r"^(\d+)-", child.name)
        if match:
            max_order = max(max_order, int(match.group(1)))
    order = max_order + 10
    return parent / f"{order:02d}-{slug}"


def create_panel(args: argparse.Namespace) -> None:
    slug = slugify(args.id)
    title = args.title or args.id
    order = args.order
    panel_dir = PANELS_ROOT / f"{order:02d}-{slug}"
    write(panel_dir / "panel.toml", md([
        f'id = "{slug}"',
        f'title = "{title}"',
        f'kind = "{args.kind}"',
        f'order = {order}',
    ]))
    write(panel_dir / "overview.md", f"# {title}\n\n")
    write(panel_dir / "context.md", f"# {title} context\n\n")
    (panel_dir / "views").mkdir(parents=True, exist_ok=True)


def create_view(args: argparse.Namespace) -> None:
    panel_dir = find_panel(args.panel)
    slug = slugify(args.id)
    title = args.title or args.id
    view_dir = next_order_dir(panel_dir / "views", slug)
    lines = [
        f'id = "{slug}"',
        f'title = "{title}"',
        f'kind = "{args.kind}"',
        "order = 0",
    ]
    if args.repo:
        lines.append(f'repo = "{args.repo}"')
    if args.generated:
        lines.append(f'generated = "{args.generated}"')
    write(view_dir / "view.toml", md(lines))
    write(view_dir / "overview.md", f"# {title}\n\nstatus: active\n")
    write(view_dir / "context.md", f"# {title} context\n\n")
    write(view_dir / "notes.md", f"# {title} notes\n\n")
    write(view_dir / "prompt.md", f"# {title} prompt\n\n")
    write(view_dir / "milestones" / "ms0.md", f"# ms0: {title} baseline\n\nstatus: active\n")
    if args.kind == "project":
        write(view_dir / "git.md", f"# {title} git\n\nGenerated git state will appear here later.\n")


def create_milestone(args: argparse.Namespace) -> None:
    panel_id, _, view_id = args.target.partition("/")
    if not panel_id or not view_id:
        raise SystemExit("target must look like panel/view, for example school/mmai")
    panel_dir = find_panel(panel_id)
    wanted = slugify(view_id)
    views_dir = panel_dir / "views"
    view_dir = None
    for child in sorted(views_dir.iterdir() if views_dir.exists() else []):
        if not child.is_dir():
            continue
        view_file = child / "view.toml"
        parsed = None
        if view_file.exists():
            parsed = parse_toml_value(view_file.read_text(encoding="utf-8"), "id")
        candidates = {slugify(child.name), slugify(re.sub(r"^\d+-", "", child.name))}
        if parsed:
            candidates.add(slugify(parsed))
        if wanted in candidates:
            view_dir = child
            break
    if view_dir is None:
        raise SystemExit(f"view not found: {args.target}")

    milestone_slug = slugify(args.name)
    write(
        view_dir / "milestones" / f"{milestone_slug}.md",
        f"# {args.name}\n\nstatus: active\n\n## goal\n\n- [ ] define goal\n",
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Add panels, views, and milestones to alpnest.")
    sub = parser.add_subparsers(dest="cmd", required=True)

    panel = sub.add_parser("panel")
    panel.add_argument("id")
    panel.add_argument("--title")
    panel.add_argument("--kind", default="generic")
    panel.add_argument("--order", type=int, default=50)
    panel.set_defaults(func=create_panel)

    view = sub.add_parser("view")
    view.add_argument("panel")
    view.add_argument("id")
    view.add_argument("--title")
    view.add_argument("--kind", default="generic")
    view.add_argument("--repo")
    view.add_argument("--generated")
    view.set_defaults(func=create_view)

    course = sub.add_parser("course")
    course.add_argument("id")
    course.add_argument("--title")
    course.set_defaults(func=lambda args: create_view(argparse.Namespace(panel="school", id=args.id, title=args.title, kind="course", repo=None, generated=None)))

    project = sub.add_parser("project")
    project.add_argument("id")
    project.add_argument("--title")
    project.add_argument("--repo")
    project.set_defaults(func=lambda args: create_view(argparse.Namespace(panel="projects", id=args.id, title=args.title, kind="project", repo=args.repo or str(Path.home() / "Documents" / "GitHub" / slugify(args.id)), generated=None)))

    job = sub.add_parser("job")
    job.add_argument("id")
    job.add_argument("--title")
    job.set_defaults(func=lambda args: create_view(argparse.Namespace(panel="job", id=args.id, title=args.title, kind="job", repo=None, generated=None)))

    milestone = sub.add_parser("milestone")
    milestone.add_argument("target", help="panel/view, for example school/mmai")
    milestone.add_argument("name")
    milestone.set_defaults(func=create_milestone)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()

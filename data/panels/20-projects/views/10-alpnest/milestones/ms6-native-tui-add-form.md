# ms6: native TUI add form

status: shelved
priority: medium
reason: correct long-term UX, but too large for today's scope

## goal

Allow users to create panels, views, courses, projects, jobs, mail-like components, and milestones from inside the TUI.

## desired UX

- `a`: open add menu
- choose add panel / add view / add milestone
- enter fields using native text input
- navigate fields with tab/shift-tab
- validate slug/title/path/kind
- create or cancel
- reload registry after creation

## implementation notes

This requires:

- Rust text input state
- cursor management
- modal state
- form field navigation
- validation messages
- create/cancel workflow
- filesystem writes from TUI
- registry reload after write

## current decision

Do not implement now.

Use the CLI creation workflow for now:

```bash
python3 scripts/alpnest-add.py panel research --title research --order 50
python3 scripts/alpnest-add.py course computer-vision --title "computer vision"
python3 scripts/alpnest-add.py project acp --repo ~/Documents/GitHub/acp
python3 scripts/alpnest-add.py milestone school/mmai bridge
```

## unblock condition

Start this only after:

- registry reload with `r` works
- local git tracker exists
- TUI readability/theme pass is stable

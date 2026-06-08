# alpnest planning prompt

Use this file as project-specific context when planning alpnest work.

## current priority

Implement alpnest features in this order:

1. registry reload with `r`
2. local git tracker
3. readability/colors/theme
4. calendar tracker and push notifications
5. project milestone drill-down views
6. native TUI add form

## current state

The dynamic panel/view registry exists.

Alpnest now reads the user profile from:

```text
data/panels/
```

The current creation workflow is CLI-based:

```bash
python3 scripts/alpnest-add.py panel research --title research --order 50
python3 scripts/alpnest-add.py course computer-vision --title "computer vision"
python3 scripts/alpnest-add.py project acp --repo ~/Documents/GitHub/acp
python3 scripts/alpnest-add.py milestone school/mmai bridge
```

## shelved feature

Native TUI add form is desired but shelved for now.

Final desired UX:

- add panel from inside TUI
- add view under current panel
- add course/job/project/mail-like component
- add milestone under current view
- Rust text input
- field navigation
- validation
- create/cancel workflow
- automatic registry reload

Do not start this until registry reload, git tracker, and theme/readability are stable.

## constraints

- Do not make alpnest a full IDE.
- Do not make alpnest a full mailbox app.
- Keep terminal available through zellij/tmux.
- Keep personal planning state under data/ uncommitted by default.
- Product code lives in src/, scripts/, prompts/, config/, README, and assets.

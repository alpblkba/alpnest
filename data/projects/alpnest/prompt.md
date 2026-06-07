# alpnest prompt

Alpnest should become a local-first terminal cockpit.

Important design constraints:
- Do not turn Alpnest into a full mailbox app.
- Do not build a terminal emulator too early.
- Prefer zellij/tmux integration for real terminal usage.
- Project data should live under data/projects/<project>/.
- Each project should have overview.md, context.md, git.md, notes.md, prompt.md, and milestones/.
- overview.md is for the TUI.
- context.md is for right-panel project context.
- notes.md is user-editable personal notes.
- prompt.md is project-specific guidance for future LLM/assistant planning.
- milestones are arbitrary and can be created/edited by assistant judgement.

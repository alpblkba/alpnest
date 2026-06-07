# alpnest context

Alpnest should become a daily cockpit, but it should not trap the user away from the shell.

Design direction:
- TUI shows planning, context, mail, school, and project state.
- Real terminal usage should continue through zellij/tmux, not a fake terminal emulator.
- Project state should be generated from local repos under ~/Documents/GitHub.
- Each project should have overview.md, context.md, git.md, notes.md, prompt.md, and milestones/.
- The right context panel should read context.md for the active view.

# ms2: project/git tracking view

status: active
deadline: Sunday, 7 June 2026
priority: high

## goal

Make project views useful by showing local git state.

## tasks

- [ ] discover project directories under ~/Documents/GitHub with .git
- [ ] map discovered repos to data/projects/<project> folders
- [ ] generate git.md for each repo
- [ ] include branch, status, remote, recent commits, and diff summary
- [ ] keep generated output readable inside TUI
- [ ] do not overbuild GitHub API integration yet

## future

- GitHub issues/PRs
- local branch health
- dirty repo warnings
- suggested next command

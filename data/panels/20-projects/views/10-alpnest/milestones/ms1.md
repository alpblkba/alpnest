# ms1: local git tracker

status: active
priority: highest

## goal

Discover local repositories under `~/Documents/GitHub` and make their git state visible inside alpnest.

## phase 1: generator script

- [ ] create `scripts/generate_project_git_views.py`
- [ ] discover direct child directories containing `.git`
- [ ] collect branch, upstream, remote, and status
- [ ] collect staged, unstaged, untracked, ahead, and behind counts
- [ ] collect recent commits
- [ ] collect short diffstat
- [ ] write `data/projects/<repo>/git.md`
- [ ] handle missing upstream cleanly
- [ ] handle detached HEAD cleanly
- [ ] handle git command errors as markdown warnings

## phase 2: project overview integration

- [ ] generate a compact repository health block
- [ ] add dirty repository summary to `data/projects.md`
- [ ] keep clean repositories visually quiet
- [ ] show dirty/unpushed repositories prominently

## phase 3: TUI integration

- [ ] add project `git.md` views where useful
- [ ] make `projects / alpnest / git` easy to open
- [ ] make generated git files readable in half-screen layout
- [ ] confirm zellij side-terminal workflow still works

## done criteria

- [ ] dirty repositories are obvious from alpnest
- [ ] each discovered repo has a readable `git.md`
- [ ] clean repos are not noisy
- [ ] generated git output is deterministic enough to inspect repeatedly
- [ ] no git write operation is performed

## test commands

```bash
python3 scripts/generate_project_git_views.py
sed -n '1,160p' data/projects/alpnest/git.md
sed -n '1,160p' data/projects.md
cargo fmt
cargo check
cargo test
```

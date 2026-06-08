# alpnest git tracker

status: active next task
milestone: ms1
priority: highest

## goal

Generate local git visibility for repositories under:

```text
~/Documents/GitHub
```

A directory qualifies as a project repository if it contains a `.git` directory.

## first implementation

Create:

```text
scripts/generate_project_git_views.py
```

The script should:

- scan direct children of `~/Documents/GitHub`
- ignore directories without `.git`
- create `data/projects/<repo>/` if missing
- write `data/projects/<repo>/git.md`
- preserve existing `overview.md`, `context.md`, `notes.md`, and `prompt.md`
- update a compact generated project git summary

## required generated information

For each repository:

- repository name
- repository path
- current branch
- upstream branch if available
- remote URL
- clean/dirty state
- staged file count
- unstaged file count
- untracked file count
- ahead/behind count if available
- recent commits
- short diffstat
- raw `git status --short` block

## output shape

Each generated `git.md` should look roughly like this:

```markdown
# git: alpnest

repo: /Users/alpblkba/Documents/GitHub/alpnest
branch: main
upstream: origin/main
remote: https://github.com/alpblkba/alpnest.git
state: dirty
ahead: 0
behind: 0

## status

text block containing git status --short

## recent commits

- abc1234 commit message

## diffstat

text block containing git diff --stat
```

## done criteria

- [ ] `python3 scripts/generate_project_git_views.py` runs without crashing
- [ ] alpnest repo gets a useful `data/projects/alpnest/git.md`
- [ ] every direct `.git` repo under `~/Documents/GitHub` gets a git view
- [ ] clean repos are compact
- [ ] dirty repos are obvious
- [ ] missing upstream does not crash the script
- [ ] git command failures are rendered as warnings, not Python crashes

## guardrails

- Read repository state only.
- Do not run `git add`, `git commit`, `git push`, `git pull`, or `git checkout`.
- Do not delete or rewrite project notes/context/prompt files.
- Generated `git.md` can be overwritten.

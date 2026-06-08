# alpnest git tracker

status: planned

milestone: ms1

## goal

Generate local git visibility for repositories under:

```text

~/Documents/GitHub

```

A directory qualifies as a project repository if it contains a `.git` directory.

## required generated information

For each repository:

- repository name

- repository path

- current branch

- upstream branch if available

- remote URL

- dirty/clean state

- staged file count

- unstaged file count

- untracked file count

- ahead/behind count if available

- recent commits

- short diffstat

- status summary

## desired files

Each known project should eventually have:

```text

data/projects/<repo>/git.md

```

The projects overview should show the most important git state, especially dirty or unpushed repositories.

## implementation preference

Start with a Python generator script because it can shell out to git quickly and write markdown projections.

Candidate script:

```text

scripts/generate_project_git_views.py

```

Later, this can move into Rust if it becomes stable enough.


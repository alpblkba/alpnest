# alpnest operating contract

## core rule

alpnest has two different kinds of files:

1. product files
2. personal working-state files

Product files can be committed.
Personal working-state files should usually not be committed.

Alp's daily plans, course plans, task lists, project TODOs, mail triage outputs, calendar-derived snapshots, and LLM-populated planning files are local working state. They may change daily and should not pollute git history.

## commit by default

Commit these when they change intentionally:

```text
src/
scripts/
prompts/
config/
assets/
README.md
Cargo.toml
Cargo.lock
.gitignore
```

These are product/code/architecture files.

Examples:

- Rust TUI implementation
- mail parser/filter logic
- Qwen prompt schema
- zellij/tmux launcher scripts
- mail sync scripts
- calendar sync scripts
- git tracker scripts
- README updates
- screenshot/assets intended for GitHub
- config templates intended for users

## do not commit by default

Do not commit these by default:

```text
data/
data/projects/
data/today.md
data/school.md
data/mail.md
data/job.md
data/**/*.context.md
data/**/overview.md
data/**/notes.md
data/**/prompt.md
data/**/git.md
data/**/milestones/*.md
```

These are Alp's personal working state.

Examples:

- today's TODOs
- MMAI study plan
- seminar reading plan
- HiWi preparation plan
- Vitis practice plan
- generated git summaries
- generated calendar snapshots
- generated mail summaries
- daily milestone edits
- personal notes
- project-specific prompt/context generated for Alp

## exception: seed data

Some `data/` files may be committed only if they are deliberately written as repo seed/demo data.

This must be explicit.

Allowed examples:

```text
data/example/
data/templates/
data/sample-project/
examples/
```

Not allowed as accidental commits:

```text
data/projects/alpnest/overview.md
data/projects/mmai/overview.md
data/projects/seminar/milestones/ms0.md
data/today.md
```

If unsure, do not commit `data/`.

## generated files

Generated files should not be committed unless they are part of the product documentation or a deliberate fixture.

Usually do not commit:

```text
~/.local/share/alpnest/
data/generated/
data/**/git.md
data/**/calendar.md
data/**/mail*.md
```

Commit only if:

- it is a test fixture
- it is a documented example
- it is intentionally anonymized/sample output

## when Alp asks for a plan

If Alp says something like:

> MMAI için bana program yap
> seminar reading plan yap
> HiWi için Vitis hazırlığı çıkar
> bugün ne yapacağımı Alpnest’e yaz

Then update the relevant local planning files but do not commit them.

Examples:

```text
data/school.md
data/school.context.md
data/projects/mmai/overview.md
data/projects/mmai/context.md
data/projects/mmai/milestones/ms0.md
data/projects/seminar/overview.md
data/projects/hiwi/overview.md
data/today.md
```

These are local cockpit state.

## when Alp asks for implementation

If Alp says something like:

> local git tracker implement edelim
> mail filters’i geliştir
> zellij layout’u düzelt
> calendar sync script’i yaz
> README’i güncelle

Then modify product files and these can be committed.

Examples:

```text
scripts/generate_project_git_views.py
scripts/sync_calendar_apple.py
scripts/alpnest-session.sh
src/main.rs
src/mail/feed.rs
prompts/qwen/mail_summarizer/task.md
README.md
```

## git add policy

Before committing, inspect:

```bash
git status --short
git diff --stat
```

Default safe add pattern:

```bash
git add src scripts prompts config README.md Cargo.toml Cargo.lock .gitignore assets
```

Avoid:

```bash
git add data
git add .
```

Never use `git add .` unless Alp explicitly confirms that personal data files are safe to include.

## commit message policy

Use product-oriented commit messages.

Good:

```text
add local git tracker generator
stabilize zellij cockpit launcher
improve mail triage schema
add qwen mail summarizer prompts
```

Bad:

```text
update my MMAI plan
add today todo
seminar tasks
hiwi preparation
```

Daily planning files should remain local.

## recommended .gitignore direction

Long term, ignore most personal state:

```gitignore
data/generated/
data/today.md
data/*.context.md
data/projects/*/overview.md
data/projects/*/context.md
data/projects/*/notes.md
data/projects/*/prompt.md
data/projects/*/git.md
data/projects/*/milestones/*.md
```

Keep templates and examples separately:

```text
data/templates/
examples/
```

## assistant behavior contract

When helping Alp with Alpnest:

1. Decide whether the task is product work or personal planning state.
2. If it is product work, edit code/config/docs and suggest commit commands.
3. If it is personal planning state, edit markdown state files and do not suggest committing them.
4. Never suggest `git add .` by default.
5. Always separate commit-worthy files from local-state files.
6. For course/project plans, populate Alpnest files but keep them uncommitted.
7. For implementation changes, run or request tests before commit.
8. If a file contains personal daily planning, assume it is local-only unless Alp explicitly says otherwise.

## practical placement

Recommended committed product document:

```text
docs/alpnest-contract.md
```

Recommended local reminder, not committed by default:

```text
data/projects/alpnest/prompt.md
```

## current interpretation

When Alp asks for course planning, daily planning, seminar planning, HiWi preparation, or MMAI scheduling, populate files under `data/` and do not recommend committing them.

When Alp asks for product implementation, modify `src/`, `scripts/`, `prompts/`, `config/`, `assets/`, or `README.md`, then recommend test and commit commands.

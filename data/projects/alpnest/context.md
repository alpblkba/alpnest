# alpnest context

alpnest is now the local sanity-check point of contact for the user's daily work.

The project is moving from a simple TUI experiment toward a local-first cockpit that coordinates mail, projects, school, job context, calendar, and terminal work.

## operating model

alpnest should be opened before starting serious work. It should summarize the state of the local system and help decide the next bounded action.

Important principle: alpnest should not replace existing tools. It should sit beside them.

- terminal remains available through zellij/tmux
- vim remains the editor for notes and prompts
- Apple Mail remains the mailbox source
- local markdown remains the inspectable planning/state layer
- Qwen/Ollama handles local summarization and low-stakes triage
- ChatGPT can provide higher-level judgement and planning

## current architecture

Stable repository files live under the project repo. Runtime/generated state lives under `~/.local/share/alpnest`.

Project folders use this shape:

```text
data/projects/<project>/
  .<project>.cfg
  overview.md
  context.md
  git.md
  notes.md
  prompt.md
  milestones/
    ms0.md
    ms1.md
    .milestones.sh
overview.md is the first visible project page.
context.md is stable background for the context panel.
git.md should be generated from local repository state.
notes.md is Alp-editable working notes.
prompt.md contains project-specific planning context for future LLM judgement.
milestones/ contains arbitrary milestone files.

design rules

* Keep generated files disposable.
* Keep stable state readable.
* Keep automation inspectable.
* Prefer local scripts before opaque services.
* Avoid destructive writes without explicit confirmation.
* Avoid notification spam.
* Prefer read-only integrations first.
* Do not let alpnest block terminal work.

feature order rationale

Local git tracker comes first because it is low-risk and immediately useful. It gives alpnest real awareness of active work without touching external services.

Readability/theme comes second because alpnest is becoming a daily-use surface. If it is visually noisy or hard to read, the system will not be used.

Calendar tracker comes third because it is extremely useful but higher-risk. The first phase must be read-only: snapshot, upcoming events, reminders, and planning context. Calendar write access comes later.

Notifications come with calendar/mail/git deltas, but only after deduplication and importance rules exist.

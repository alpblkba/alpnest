# alpnest

status: active
role: sanity-check point of contact
current phase: dynamic registry baseline finished

## purpose

alpnest is the local cockpit before work starts.

It should become the first place to check:

- today's tasks
- active school/course work
- project state
- job/HiWi context
- important mail
- upcoming calendar events
- local git/repository health

The goal is not to replace the terminal, editor, mailbox, or calendar. The goal is to give one calm local surface that tells Alp what needs attention before opening the rest of the toolchain.

## completed baseline

- [x] Rust/Ratatui TUI exists.
- [x] panels exist for today, school, projects, job, and mail.
- [x] alpnest can be installed as a global command.
- [x] zellij/tmux session launcher exists so alpnest runs beside a real terminal.
- [x] local mail sync through Apple Mail exists.
- [x] local Qwen/Ollama mail summarization exists.
- [x] deterministic mail filters exist.
- [x] mail triage schema exists with category, attention, importance, and retention fields.
- [x] generated mail overview, KIT mail, Gmail mail, and detail projections exist.
- [x] hidden spam/promotion/social mail is mostly filtered out of the main attention feed.
- [x] project folders now have overview, context, git, notes, prompt, and milestones structure.
- [x] dynamic `data/panels` registry exists.
- [x] panels/views are no longer hardcoded as school/projects/job/mail in Rust.
- [x] `scripts/alpnest-add.py` exists for CLI-based panel/view/milestone creation.
- [x] tests pass after dynamic registry migration.

## current TODO

### active next

- [ ] make `r` reload the dynamic panel registry without restarting alpnest
- [ ] stabilize `scripts/alpnest-add.py`
- [ ] add tests for panel/view/milestone creation through the registry
- [ ] start local git tracker after reload path is stable

### next product features

1. registry reload with `r`
2. local git tracker
3. readability, colors, and theme polish
4. calendar tracker and push notifications
5. project milestone drill-down views
6. native TUI add form

## shelved

### native TUI add form

status: shelved, not cancelled

The desired final UX is native TUI creation:

- add panel
- add view under current panel
- add course/job/project/mail-like component
- add milestone under current view
- text input fields
- field navigation
- validation
- create/cancel workflow
- automatic registry reload after creation

This is the right long-term design, but it is larger than today's scope. For now, use `scripts/alpnest-add.py` as the stable creation path.

## active milestones

### ms1: dynamic panel/view registry

priority: done
status: completed baseline

The app now reads panels and views from `data/panels`.

### ms2: local git tracker

priority: highest next
status: planned

Build local repository state summaries for project views.

### ms3: readability and theme pass

priority: high
status: planned

Improve contrast, colors, labels, borders, and half-screen usability.

### ms4: calendar tracker and push notifications

priority: high
status: planned

Start with read-only calendar snapshots and sparse notifications.

### ms5: project milestone drill-down

priority: medium
status: planned

Make milestone files first-class navigable detail views.

### ms6: native TUI add form

priority: medium
status: shelved

Implement later after registry reload, git tracker, and theme pass.

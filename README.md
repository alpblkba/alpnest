# alpnest

A local-first terminal cockpit for tasks, school, projects, mail, and working context.

Built with Rust, Ratatui, Apple Mail scripts, local markdown stores, Ollama/Qwen, and a Zellij-first terminal workflow.

![alpnest beta](assets/alpnest_beta_upscaled.jpg)

## contents

- [what this is](#what-this-is)
- [current shape](#current-shape)
- [layout and zellij workflow](#layout-and-zellij-workflow)
- [local data model](#local-data-model)
- [mail pipeline](#mail-pipeline)
- [mail filtering and triage](#mail-filtering-and-triage)
- [qwen summarization contract](#qwen-summarization-contract)
- [scripts](#scripts)
- [scheduled sync and notifications](#scheduled-sync-and-notifications)
- [project and context model](#project-and-context-model)
- [running locally](#running-locally)
- [roadmap](#roadmap)
- [tracelog](#tracelog)

## what this is

alpnest is a personal terminal home screen. It sits before the work starts: a compact place to inspect the day, active school work, local repositories, mail signals, and context files before deciding what to do next.

It is not trying to become a full mail client, a code editor, or an autonomous agent. The design goal is smaller and stricter: keep local state readable, keep automation inspectable, and keep the terminal usable.

The current direction is a cockpit rather than an app launcher. alpnest should help answer:

- what needs attention now?
- which school/job/project context matters today?
- which mails are worth seeing, and which ones should disappear?
- what local repository state should I notice before working?
- what should be handed to an LLM for judgement, planning, or decomposition?

## current shape

The TUI currently exposes top-level panels for:

- `today`
- `school`
- `projects`
- `job`
- `mail`

The repository contains fallback markdown files under `data/`, while runtime views are generated under `~/.local/share/alpnest/`. This keeps the program bootable even when no sync process has run yet.

The current build supports:

- nested panel/view navigation
- generated mail overview, KIT mail, and Gmail mail views
- full mail detail views backed by locally fetched Apple Mail bodies
- transient detail views that do not mutate the stable mail overview tab
- local mail event streams grouped by account, sender, subject, and message chain
- deterministic filtering before local model calls
- local Qwen-based summarization and triage through Ollama structured output
- attention-aware mail placement: overview, account-only, or hidden
- git/project-oriented local markdown structure
- zellij/tmux session launcher so the TUI does not take over the shell

## layout and zellij workflow

alpnest is intended to run next to a real shell, not instead of one.

The preferred launcher is:

```sh
alpnest
```

When run outside an existing terminal multiplexer, alpnest can launch a Zellij session through `scripts/alpnest-session.sh`. The layout keeps alpnest on the left and a normal login shell on the right. This makes the TUI a cockpit while the terminal remains available for real work.

The current Zellij layout lives in:

```text
scripts/alpnest.kdl
```

For debugging or running the raw TUI without opening a session:

```sh
ALPNEST_NO_SESSION=1 alpnest
```

The same guard is used inside the Zellij/tmux launcher to prevent recursive session creation.

## local data model

Runtime data is kept outside the repository:

```text
~/.local/share/alpnest/
```

The rough structure is:

```text
~/.local/share/alpnest/
  store/
    messages.json
    eventstreams.json
    mail_sync_state.json
  generated/
    mail_feed.md
    mail_kit.md
    mail_gmail.md
    mail/
      feed/
        mail0.md
        mail1.md
        ...
      kit/
        kit0.md
        kit1.md
        ...
      gmail/
        gmail0.md
        gmail1.md
        ...
      feed_index.json
  raw/
    mail/messages/
      gmail/
      kit/
  logs/
    mail-sync.log
```

The important rule is that generated `mail0.md`, `kit0.md`, and `gmail0.md` files are projections, not identity. Stable identity lives in message IDs and thread/event stream IDs.

## mail pipeline

The mail pipeline is local and file-backed:

```text
Apple Mail
  -> scripts/sync_mail_apple.py
  -> store/messages.json
  -> store/eventstreams.json
  -> scripts/summarize_mail_local.py
  -> src/mail/* Rust feed builder and renderer
  -> generated markdown views
  -> alpnest TUI
```

Apple Mail is used as the source of truth for mailbox access. The sync script can run metadata-only or fetch full body text. Full body text is stored under `~/.local/share/alpnest/raw/mail/messages/` and rendered only when a detail view is opened.

The TUI view is intentionally compact:

- overview shows only attention-worthy mail
- KIT and Gmail views show account-specific filtered mail
- clicking a row opens a full local detail projection
- summaries are short and English by contract
- omitted detail stays available through the raw body path

## mail filtering and triage

Mail filtering has two layers.

The first layer is deterministic and configured in:

```text
scripts/mail_filters.cfg
```

This file is used to suppress senders, subjects, bodies, and known low-value patterns before they pollute the overview. It supports both the newer flat rule style and older ignore sections.

The second layer is triage metadata produced by the summarizer:

```json
{
  "category": "assignment|project|career|security|travel|promotion|noise|...",
  "attention": "overview|account_only|hidden",
  "importance": "high|medium|low",
  "retention_hint": "24h|3d|7d|until_deadline|keep|hidden"
}
```

The overview is intentionally strict. Promotions, shopping mail, newsletters, social updates, and obvious noise should not appear there. Account views can remain broader, but hidden mail should stay hidden there too.

This is still a heuristic system. The long-term goal is not perfect email classification; it is a useful attention feed that does not waste the first minutes of the day.

## qwen summarization contract

Local summarization is handled by:

```text
scripts/summarize_mail_local.py
prompts/qwen/mail_summarizer/
```

The target model is currently `qwen3:8b` through Ollama HTTP structured output.

The prompt pack is split into:

```text
prompts/qwen/mail_summarizer/
  system.md
  task.md
  output_schema.md
  examples.md
  failure_modes.md
  rubric.md
  context.md
  README.md
```

The contract is stricter than a normal summary prompt. The model must return structured JSON with a short English summary, cleaned sender/subject, category, attention decision, importance, retention hint, action/deadline fields, language metadata, noise flag, human-review flag, and confidence.

The current philosophy:

- Qwen handles cleanup, short summaries, and low-stakes classification.
- Deterministic rules catch obvious noise before model calls when possible.
- ChatGPT or another higher-level model should handle judgement, planning, and decomposition later.
- Non-English mail should still summarize into English.
- Ambiguous, official, or action-bearing mail should prefer review over overconfident hiding.

This makes the local model useful without pretending it is the final planner.

## scripts

The current script surface:

```text
scripts/sync_mail_apple.py
  Pulls recent Apple Mail messages into the local store. Can fetch body text.

scripts/summarize_mail_local.py
  Runs deterministic triage and optional Qwen/Ollama summarization.

scripts/generate_mail_view.py
  Legacy/generated mail view helper kept around during the transition.

src/bin/generate_mail_feed.rs
  Rust mail feed renderer for overview/account/detail projections.

scripts/alpnest-session.sh
  Starts alpnest with a side terminal using Zellij first, tmux as fallback.

scripts/alpnest-mail-sync.sh
  Syncs mail, summarizes, regenerates feed output, and can trigger notifications.

scripts/alpnestd.py
  Earlier daemon runner for periodic local automation.

scripts/alpnestd.cfg
  Conservative daemon configuration.
```

The Rust side owns the TUI and the newer feed rendering path. The Python side owns collection, Apple Mail integration, and local model prompting.

## scheduled sync and notifications

The mail sync wrapper is:

```text
scripts/alpnest-mail-sync.sh
```

It performs the practical sync chain:

```text
sync Gmail/KIT from Apple Mail
  -> summarize recent streams
  -> regenerate Rust mail feed
  -> hash generated overview
  -> notify if the attention feed changed
```

On macOS, the intended scheduler is `launchd`, using a user LaunchAgent such as:

```text
~/Library/LaunchAgents/com.alp.alpnest.mail-sync.plist
```

The current notification mechanism uses `osascript` and macOS notifications. The useful next refinement is to notify only when a new or changed `attention=overview` item appears, rather than on any feed hash change.

## project and context model

The local project model is moving toward paired markdown contexts:

```text
data/
  today.md
  today.context.md
  school.md
  school.context.md
  projects.md
  projects.context.md
  job.md
  job.context.md
  mail.md
  mail.context.md
```

Project-like entries are becoming folders:

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
```

The intended split is:

- `overview.md`: what the TUI should show first
- `context.md`: stable background context for the right/bottom context pane
- `git.md`: generated local repository state
- `notes.md`: user notes, not necessarily instruction-bearing
- `prompt.md`: project-specific planning context for future LLM review
- `milestones/`: arbitrary milestone files, not a fixed task count

School/praktikum overlap is allowed. For example, `iot-lab` and `hardware-security` can appear under school and projects because they are both course work and local repositories.

## running locally

Install the binary:

```sh
cargo install --path . --force
```

Run with session integration:

```sh
alpnest
```

Run without Zellij/tmux session launch:

```sh
ALPNEST_NO_SESSION=1 alpnest
```

Run checks:

```sh
cargo fmt
cargo check
cargo test
```

Generate the mail feed manually:

```sh
cargo run --bin generate_mail_feed
```

Run the mail sync wrapper manually:

```sh
scripts/alpnest-mail-sync.sh
```

## roadmap

Near-term:

- make the right/bottom context pane read the active sibling `context.md`
- generate project `git.md` files from repositories under `~/Documents/GitHub`
- improve Zellij layout and session ergonomics
- reduce mail notification noise to attention-worthy deltas
- make mail retention/deadline handling real instead of just metadata
- add calendar snapshots and calendar-aware planning
- separate course, praktikum, project, and job views cleanly

Later:

- GitHub issue/PR status in project views
- local branch health and dirty repo warnings
- editable notes/prompt flows through Vim
- review packet generation for ChatGPT planning sessions
- task promotion from mail/calendar/project context
- controlled handoff format for LLM planning rather than hidden automation

## tracelog

- 2026-06-07: added Zellij/tmux session workflow and started treating alpnest as a cockpit beside a real terminal.
- 2026-06-07: reshaped project/school/job data into folder-backed markdown contexts and milestone files.
- 2026-06-05: tightened mail filtering and attention triage so low-value promotions and social noise do not enter the main overview.
- 2026-06-05: added Rust mail feed/detail views with stable thread identity and disposable generated slot projections.
- 2026-05-28: added the local Qwen mail cockpit pipeline and expanded the summarizer prompt contract.
- 2026-05-27: added early local daemon and mail decomposition snapshot pipeline.

alpnest is still early. The useful part is not that it is complete; it is that the shape is now clear enough to keep iterating without pretending it is a finished productivity system.

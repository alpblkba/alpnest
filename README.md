# alpnest

My personal terminal nest for tasks, projects, school, mail, and context.

Built with Rust and Ratatui.

![alpnest beta](assets/alpnest_beta_upscaled.png)

## current shape

alpnest is a small local-first TUI that acts as a home screen before work starts. It is not a code editor, not a mail client replacement, and not an automated agent. It is a compact place to inspect local state before deciding what to do next.

The current version supports:

- top-level panels for `today`, `school`, `projects`, and `mail`
- nested views inside panels
- generated local mail views for account-level mail inspection
- Apple Mail sync through local scripts, with optional body fetching
- separate Gmail and KIT mail digests
- local mail event streams grouped by account, sender, and subject
- deterministic fallback and obvious-noise skipping before local model calls
- optional local mail summarization and light classification through Ollama HTTP structured output, currently targeted at `qwen3:8b`
- metadata-first mail ingestion, with body fetching kept optional
- generated markdown views under the local alpnest data directory
- fallback repository markdown files when generated local data does not exist

## local data model

alpnest keeps generated runtime data outside the repository, under:

```text
~/.local/share/alpnest/
```

The current local structure is roughly:

```text
~/.local/share/alpnest/
  store/
    messages.json
    eventstreams.json
    tasks.json
    mail_sync_state.json
  generated/
    mail.md
    mail_kit.md
    mail_gmail.md
    mail_decomposition.md
    today.md
    calendar.md
  raw/
    mail/messages/
  logs/
    alpnestd.log
```

The repository still contains simple fallback markdown files under `data/`, so the TUI can open even before local automation has generated anything.

## mail pipeline

The current mail pipeline is deliberately simple:

```text
Apple Mail
  -> sync_mail_apple.py
  -> messages.json
  -> eventstreams.json
  -> summarize_mail_local.py
  -> generate_mail_view.py
  -> generated mail views
  -> alpnest TUI
```

The detailed review-oriented output is kept separate from the human-readable mail panel output:

```text
generated/mail.md
  compact account overview

generated/mail_kit.md
  KIT mail digest

generated/mail_gmail.md
  Gmail mail digest

generated/mail_decomposition.md
  structured snapshot for later review and planning
```

The summarizer is optional. If Ollama or the configured local model is not available, the system falls back to deterministic summaries and categories. Obvious noise can be skipped before model calls.

## daemon

The local daemon is configured through:

```text
scripts/alpnestd.cfg
```

The current default behavior is conservative:

- run every 5 minutes
- sync a small number of recent messages per account
- keep mail body fetching disabled by default
- keep local summarization disabled unless explicitly enabled

## plans

The next direction is to make alpnest a configurable local cockpit rather than a hardcoded TUI.

Planned work:

- move panel and nested-view definitions into a config file
- support user-editable panel/view creation from inside the TUI
- add proper task files and task promotion from mail/calendar/project events
- add calendar snapshots and calendar-aware planning views
- add a review packet generator for manual ChatGPT planning sessions
- keep ChatGPT-based judgement, decomposition, and task assignment manual rather than fully automated
- use a contract-like local handoff format for ChatGPT review instead of direct API automation
- keep local models responsible for low-stakes cleanup such as summarization, classification, and formatting
- keep higher-level judgement, priority, urgency, value, and motivation review separate
- eventually connect selected tasks to external notes, probably through a companion notes/editor workflow

The intended split is:

```text
local scripts:
  collect, normalize, summarize, render

alpnest TUI:
  inspect, navigate, select, and prepare work

local LLM:
  clean up mail and generate short summaries

ChatGPT review:
  judgement, prioritization, decomposition, and planning
```

## status

This is an early personal project. The structure is changing quickly, but the current focus is clear: keep the system local-first, boring, inspectable, and useful before adding heavier automation.

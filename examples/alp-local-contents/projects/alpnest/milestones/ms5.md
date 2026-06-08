# ms5: daemon hardening and retention logic

status: planned
priority: medium

## goal

Make background sync safe and boring.

## tasks

- [ ] add lockfiles to sync scripts
- [ ] improve logs
- [ ] prevent overlapping mail sync runs
- [ ] implement meaningful notification deltas
- [ ] apply retention metadata to generated feed visibility
- [ ] separate hidden/account-only/overview retention behavior
- [ ] document launchd setup

## done criteria

- [ ] background sync does not spam
- [ ] background sync does not overlap
- [ ] generated state stays fresh
- [ ] retention decisions are inspectable

# ms1: dynamic panel/view registry

status: baseline done
priority: completed

## completed

- [x] introduced `data/panels` as the canonical panel/view registry
- [x] migrated today, school, projects, mail, and job into registry shape
- [x] made Rust load panels/views from filesystem
- [x] generated child views from overview/context/notes/prompt/git/milestones
- [x] added `scripts/alpnest-add.py`
- [x] updated tests for dynamic registry paths
- [x] cargo test passes

## follow-up

- [ ] make `r` reload registry without restart
- [ ] add tests for dynamically added panel/view/milestone
- [ ] document the registry shape

# ms1: daily cockpit integration

status: active
deadline: Sunday, 7 June 2026
priority: high

## goal

Make Alpnest usable enough that it can become part of the daily workflow instead of remaining a prototype.

## tasks

- [ ] keep nested project filesystem stable
- [ ] verify project views resolve to data/projects/<project>/overview.md
- [ ] verify right context panel reads context.md
- [ ] keep mail overview limited to attention-worthy items
- [ ] add MMAI, seminar, and HiWi as first-class project spaces
- [ ] prepare next step: project git snapshot generator
- [ ] avoid building a terminal emulator inside Alpnest for now

## notes

Alpnest should be a terminal cockpit, not a replacement for the terminal. The likely direction is zellij/tmux integration with a real shell next to the TUI.

# TabMux documentation

TabMux is a terminal multiplexer with tabs and split panes, written in Rust on
top of ratatui/crossterm, portable-pty and vt100. It runs as a single binary,
works with the mouse as well as the keyboard, and can open a whole layout of
tabs, splits and commands from a TOML workspace file.

## User guide

| Page                                        | Contents                                                        |
| ------------------------------------------- | --------------------------------------------------------------- |
| [Getting started](getting-started.md)       | Building, running, command line, a tour of the screen           |
| [Keybindings](keybindings.md)               | The `^b` prefix, every key, and keys inside each mode           |
| [Mouse](mouse.md)                           | Buttons, resizing, drag-and-drop, selection, copy / paste       |
| [Features](features.md)                     | Zoom, wide mode, multicursor, search, prompt jumps, bell        |
| [Workspace files](workspace.md)             | Full TOML schema, profiles, path rules, examples                |
| [Shell integration](shell-integration.md)   | Prompt marks (OSC 133), how zsh / bash are set up, other shells |

## Developer guide

| Page                                | Contents                                                     |
| ----------------------------------- | ------------------------------------------------------------ |
| [Architecture](architecture.md)     | Event loop, state, layout tree, view / hit-testing, panes    |
| [Contributing](contributing.md)     | Build, checks, tests, code conventions                       |

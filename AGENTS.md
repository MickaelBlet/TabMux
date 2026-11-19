# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

TabMux: terminal multiplexer (tabs + split panes) in Rust (edition 2024), built on ratatui/crossterm, portable-pty and vt100. Single binary crate, no lib target.

## Commands

```sh
scripts/build.sh            # release build -> target/release/tabmux
scripts/build.sh --debug    # debug build
scripts/build.sh --check    # fmt --check + clippy -D warnings + tests, then build
cargo test                  # all unit tests (inline #[cfg(test)] modules)
cargo test <name>           # single test / filter, e.g. cargo test layout::tests
cargo run -- -w examples/rust-dev.toml   # run with a workspace file
```

`--check` runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

## Architecture

Event loop (`main.rs`): one `mpsc` channel of `AppEvent` (`event.rs`). An input thread forwards crossterm events; each pane's PTY reader thread sends `PtyOutput` / `PtyExit(id)`. The loop does `app.refresh()` → `ui::draw` → block on an event → `app.handle()`, then drains pending events before redrawing (coalescing). When a bell is pending it uses `recv_timeout(ui::BLINK)` to animate.

Core state (`app.rs`, `App`): `panes: HashMap<PaneId, Pane>` owns all PTYs globally; `tabs: Vec<Tab>` each hold a layout tree referencing pane IDs. Panes can therefore move between tabs (drag-and-drop) without respawning. `Mode` (Normal / Prefix / Rename / Help / Search) drives keyboard routing; prefix key is `^b` (tmux-style). `Drag` tracks the active mouse gesture.

Layout (`layout.rs`): binary tree `Node::Leaf(PaneId) | Node::Split { dir, ratio, a, b }`. Nodes are addressed by `Path = Vec<bool>` (false = first child). Workspace files describe n-ary splits with weights (`workspace.rs`); they are converted to nested binary splits at load time.

View / hit-testing (`view.rs`): `View::build(app)` is recomputed in every `refresh()` and holds every rect on screen (sidebar, top bar, pane views, separators, buttons) plus a hit list `(Rect, Target)`. Mouse handling in `app.rs` resolves clicks via `view.hit(pos)` → `Target`; later-pushed hits win. `ui.rs` only renders from `App` + `View` — keep geometry in `view.rs`, drawing in `ui.rs`. Tooltips and the help screen (`HELP_LINES`) live in `view.rs`; update them when adding keybindings/buttons.

`refresh()` is also where every PTY is resized to its current content rect (`Pane::resize` is a no-op when unchanged), bell/activity flags are collected, and search results are recomputed if dirty.

Pane (`pane.rs`): spawns `$SHELL` in a PTY, feeds output into a `vt100::Parser<Callbacks>` behind `Arc<Mutex<_>>`. `Callbacks` captures window title, bell, output activity and OSC 133;A prompt marks. "Wide mode" makes the PTY `WIDE_COLS` (1000) wide with horizontal scrolling. `SpawnOpts.command` is typed into the shell, which stays alive after it exits.

Prompt marks (`marks.rs`, `shell.rs`): `shell.rs` auto-injects integration scripts (`src/shell/*`, embedded via `include_str!`, written to `$TMPDIR/tabmux-$USER`) so zsh (via `ZDOTDIR` redirection) and bash (via `--rcfile`) emit OSC 133;A without touching user config. Disable with `TABMUX_NO_SHELL_INTEGRATION=1`. Marks are stored as absolute line numbers; `pane::feed` counts lines evicted from a full scrollback to keep them aligned.

Other modules: `input.rs` (KeyEvent → PTY bytes), `mouse.rs` (mouse forwarding to apps that request it; shift bypasses), `selection.rs` (text selection, copy via OSC 52), `search.rs` (search over scrollback + screen).

## Workspace files

TOML; schema and semantics are documented in `docs/workspace.md` (examples in `examples/`). Parsing uses `serde(deny_unknown_fields)` — unknown keys are errors. Paths: `~` expands to `$HOME`, relative paths resolve against the workspace file's directory. Keep `docs/workspace.md` in sync when changing the schema. User/dev docs live in `docs/`; update them with behavior changes (keys: `docs/keybindings.md`).

# Contributing

## Build and run

```sh
scripts/build.sh                          # release build
scripts/build.sh --debug                  # debug build
cargo run -- -w examples/rust-dev.toml    # run with a workspace
```

## Checks

Run this before committing:

```sh
scripts/build.sh --check
```

It runs, in order:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings` (every warning is an error)
3. `cargo test`
4. the build

## Tests

Unit tests live in inline `#[cfg(test)]` modules next to the code.

```sh
cargo test                     # everything
cargo test layout::tests       # one module
cargo test examples_parse      # one test
```

Main test areas: layout tree edits and geometry (`layout.rs`), view
geometry and tooltips (`view.rs`), workspace parsing / validation and every
file in `examples/` (`workspace.rs`), search, selection, mouse encoding,
prompt marks, key encoding.

## Conventions

- **Geometry in `view.rs`, drawing in `ui.rs`.** Every rect that is drawn
  or clicked is computed in `View::build` and registered in the hit list.
  `ui.rs` must not compute layout itself.
- **Hit order matters**: later-pushed hits win. Push buttons after the area
  they sit on.
- **New keybinding or button** → update `HELP_LINES` and `Target::tooltip`
  in `src/view.rs`, and [keybindings.md](keybindings.md).
- **Workspace schema change** → keep `serde(deny_unknown_fields)`, add
  validation in `Workspace::parse` if needed, update
  [workspace.md](workspace.md), and add or adjust an example in `examples/`
  (the test suite parses all of them).
- **Shell integration scripts** (`src/shell/*`) must source the user's own
  config and must never write to it.
- Code, identifiers, comments and commit messages are in English.

## Documentation

User and developer docs live in [`docs/`](README.md). Update the matching
page in the same change as the behavior.

Images live in `docs/img/` as SVG. `screen.svg`, `topbar.svg`, `help.svg`,
`search.svg` and `multicursor.svg` are real captures: after a UI change,
regenerate them with `scripts/screenshots.sh` (needs `tmux` and
`python3`). Diagrams such as `event-loop.svg` are hand-written SVG.

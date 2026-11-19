# Workspace files

A workspace is a TOML file that describes tabs, their split layouts, and the
command run in each pane. You can also add shell **profiles** for the
new-tab menu. Load one with:

```sh
tabmux -w path/to/workspace.toml
```

## Full example

```toml
active = 0                      # initially active tab (default: 0)
titles = true                   # show pane title rows (default: true)

[[tab]]
name = "code"
cwd = "~/proj"                  # default cwd for every pane in the tab
env = { RUST_BACKTRACE = "1" }  # merged into every pane's env

[tab.layout]
split = "vertical"              # side by side

[[tab.layout.panes]]
size = 2                        # 2/3 of the width
command = "nvim ."
focus = true

[[tab.layout.panes]]
split = "horizontal"            # stacked
panes = [
  { command = "cargo build" },
  { command = "cargo test", env = { RUST_LOG = "debug" } },
]

[[tab]]
name = "shell"                  # no layout: a single plain shell

[[profile]]
name = "fish"
shell = ["/usr/bin/fish", "-l"]
```

## Schema

### Top level

| Key       | Type                | Default | Description                                         |
| --------- | ------------------- | ------- | --------------------------------------------------- |
| `active`  | integer             | `0`     | Index (0-based) of the tab shown at startup. Clamped to the last tab. |
| `titles`  | bool                | `true`  | Show pane title rows at startup (toggle with `^b T`). |
| `tab`     | array of tables     | —       | **Required, at least one** `[[tab]]`.               |
| `profile` | array of tables     | `[]`    | Shell profiles for the `[⌵]` menu.                  |

### `[[tab]]`

| Key      | Type           | Description                                                      |
| -------- | -------------- | ---------------------------------------------------------------- |
| `name`   | string         | Tab title. If not set, the tab follows its focused pane's title. |
| `cwd`    | string (path)  | Default working directory for the tab's panes.                   |
| `env`    | table          | Environment variables for every pane in the tab.                 |
| `layout` | node           | Root layout node (a split or a pane). If not set, the tab gets one plain shell. |

### Layout nodes

A node is a **split** or a **pane**. A node that has a `split` key is a
split, and anything else is a pane.

#### Split node

| Key     | Type                | Description                                                |
| ------- | ------------------- | ---------------------------------------------------------- |
| `split` | `"vertical"` \| `"horizontal"` | `vertical` = children **side by side**, `horizontal` = children **stacked**. |
| `panes` | array of nodes      | Children (splits or panes), **at least one**.              |
| `size`  | number              | Weight among its siblings (default `1`, must be `> 0`).    |

#### Pane node

| Key       | Type          | Description                                                       |
| --------- | ------------- | ----------------------------------------------------------------- |
| `command` | string        | Typed into the pane's shell at startup, followed by Enter. The shell stays open after the command exits. |
| `cwd`     | string (path) | Working directory, overriding the tab's `cwd`.                    |
| `env`     | table         | Extra variables. They override the tab's `env` entries with the same name. |
| `focus`   | bool          | Focus this pane when the tab opens. Default: the first pane. If several panes set it, the first one in order wins. |
| `size`    | number        | Weight among its siblings (default `1`, must be `> 0`).           |

An empty table `{}` is a plain shell pane.

### `[[profile]]`

Profiles add a `[⌵]` button next to `[+]`. It opens a menu that starts a new
tab with the chosen profile. Profiles don't depend on tabs, so tab `cwd` and
`env` don't apply to them.

| Key       | Type             | Description                                                 |
| --------- | ---------------- | ----------------------------------------------------------- |
| `name`    | string           | Menu label. **Required, non-empty, unique.**                |
| `shell`   | array of strings | Program and arguments to run instead of `$SHELL` (non-empty). |
| `cwd`     | string (path)    | Working directory.                                          |
| `env`     | table            | Environment variables.                                      |
| `command` | string           | Typed into the shell at startup.                            |

```toml
[[profile]]
name = "fish"
shell = ["/usr/bin/fish", "-l"]   # program + args, replaces $SHELL

[[profile]]
name = "htop"
cwd = "~"
env = { TERM = "xterm" }
command = "htop"
```

If a profile fails to start (for example, the program doesn't exist), the
error shows in the status line and no tab is opened.

## Semantics

### Sizes

Sibling weights are relative. In a split with children of size `2` and `1`,
the first child gets 2/3 of the space. The layout engine only has two-way
splits, so a split with N children becomes a chain of nested two-way splits
that keeps the same proportions. The ratio of each two-way split is kept
between 10 % and 90 %, so very uneven weights (more than 9:1) get clamped.

### Paths

- `~` and `~/…` expand to `$HOME`.
- Relative paths are resolved against **the directory of the workspace
  file**, not the current directory. This makes workspaces portable:
  `cwd = ".."` in `examples/rust-dev.toml` points to the repository root.
- A pane without any `cwd` starts in the directory TabMux was launched from.

### Environment

The final environment of a pane is built in this order (later entries win):
the inherited environment, then `TERM=xterm-256color` and `TABMUX=1`, then
the tab's `env`, then the pane's `env`.

Values are passed through as-is: `$VAR` inside a value is **not** expanded
in `env`. A `command`, on the other hand, is typed into the shell, so the
shell expands it (`command = "${EDITOR:-vi} ."` works).

### Validation

Parsing is strict. TabMux refuses to start and reports the error when:

- a key is unknown, at any level (`deny_unknown_fields`). Typos are caught;
- there is no `[[tab]]`;
- a split has no `panes`;
- a `size` is zero, negative, or not a finite number;
- a profile's `name` is missing, empty or a duplicate, or its `shell` is an
  empty array.

Error messages include the location, e.g. `tab[0].layout.panes[1]: size must be > 0`.

## Examples

All files in [`examples/`](../examples) are parsed by the test suite
(`workspace::tests::examples_parse`).

### `minimal.toml` — two plain tabs

```toml
[[tab]]
name = "main"

[[tab]]
name = "scratch"
```

### `rust-dev.toml` — editor + build/test, git tab, profiles

A `code` tab with the editor on the left (2/3 of the width) and
`cargo build` / `cargo test` stacked on the right, plus a `git` tab showing
the log graph. `cwd = ".."` is relative to the file, so it is the repository
root. It also defines two profiles: a plain `/bin/sh`, and `cargo watch`.

```sh
cargo run -- -w examples/rust-dev.toml
```

### `monitoring.toml` — 2×2 grid

`titles = false` hides the title rows. A horizontal split holds two vertical
splits: `top`, `df`, `free` and `tail -f /var/log/syslog` (focused).

### `web-stack.toml` — one tab per service

- `backend`: `npm run dev` (3/4 of the height) above a focused shell, with
  `PORT` and `APP_ENV` set.
- `frontend`: dev server, test watcher and a shell, side by side with
  weights 1 / 1 / 2.
- `db`: a `psql` session.

`active = 1` starts on `frontend`.

## Tips

- One shell per tab with no layout: just list `[[tab]]` entries with a `name`.
- To get a grid, nest splits in opposite directions.
- Use a pane with no `command` and `focus = true` as your working shell
  next to long-running processes.

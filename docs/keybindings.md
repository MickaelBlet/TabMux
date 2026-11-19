# Keybindings

TabMux uses a tmux-style **prefix key: `Ctrl+b`** (written `^b`). Press
`^b`, let go, then press the command key. Any other key is sent to the
focused pane, or to every pane of the tab while
[multicursor](features.md#multicursor) is on.

`^b ?` opens the help screen, which lists the same bindings. Press any key or
click anywhere to close it.

## Prefix commands (`^b` then …)

### Tabs

| Key     | Action                                                   |
| ------- | -------------------------------------------------------- |
| `c`     | New tab                                                  |
| `&`     | Close the current tab and all of its panes               |
| `n`     | Next tab (wraps around)                                  |
| `p`     | Previous tab (wraps around)                              |
| `1`–`9` | Go to tab 1–9                                            |
| `r`     | Rename the current tab (see [Rename mode](#rename-mode)) |

### Panes

| Key                   | Action                                                       |
| --------------------- | ------------------------------------------------------------ |
| `%` or `\|`           | Split the focused pane side by side (new pane on the right)  |
| `"` or `-`            | Split the focused pane top / bottom (new pane below)         |
| `o`                   | Focus the next pane in the tab                               |
| `←` `↑` `↓` `→` / `h` `k` `j` `l` | Focus the nearest pane in that direction         |
| `z`                   | Zoom / unzoom the focused pane                               |
| `w`                   | Toggle [wide mode](features.md#wide-mode) (1000 columns)     |
| `m`                   | Toggle [multicursor](features.md#multicursor) for the tab    |
| `x`                   | Close the focused pane                                       |

### History

| Key       | Action                                                                  |
| --------- | ----------------------------------------------------------------------- |
| `/`       | [Search](features.md#search) the focused pane's scrollback              |
| `[` / `]` | Jump to the previous / next [shell prompt](features.md#prompt-navigation) |

### Interface

| Key       | Action                                       |
| --------- | -------------------------------------------- |
| `<` / `>` | Shrink / grow the sidebar by 2 columns       |
| `s`       | Show / hide the tab list                     |
| `t`       | Switch between sidebar and top tab bar       |
| `T`       | Show / hide pane title rows                  |
| `?`       | Help screen                                  |
| `q`       | Quit TabMux (closes every pane)              |
| `^b`      | Send a literal `^b` to the pane(s)           |

Any other key after the prefix is ignored.

## Keys without the prefix

| Key                    | Action                                                                  |
| ---------------------- | ----------------------------------------------------------------------- |
| `Ctrl+PgUp` / `Ctrl+PgDn` | Focus the previous / next pane across **all** tabs, in sidebar order |
| `Shift+PgUp` / `Shift+PgDn` | Scroll the focused pane by one page (keeps one line of overlap)  |
| `Ctrl+↑` / `Ctrl+↓`    | Jump to the previous / next shell prompt                                |

`Shift+PgUp/PgDn` and `Ctrl+↑/↓` are only caught while the pane is on its
normal screen. While a full-screen program (vim, less, htop, …) uses the
alternate screen, these keys go to that program.

Typing anything else sends it to the pane. Typing also scrolls the pane back
to the bottom, clears the selection and, in wide mode, brings the cursor back
into view.

## Mode keys

The current mode is shown in the status area.

### Rename mode

Started with `^b r`, a double-click on a tab, or the `✎` button.

| Key         | Action                                                |
| ----------- | ----------------------------------------------------- |
| text        | Edit the name (pasted text is accepted, newlines become spaces) |
| `Backspace` | Delete the last character                             |
| `Ctrl+u`    | Clear the name                                        |
| `Enter`     | Save the name                                         |
| `Esc`       | Cancel                                                |

An empty name makes the tab follow its focused pane's title again. Clicking
anywhere also saves the name.

### Search mode

Started with `^b /` or the `[⌕]` button. The search bar takes over the pane's
title row.

| Key            | Action                                  |
| -------------- | --------------------------------------- |
| text           | Edit the query (results update as you type) |
| `Enter` / `↑`  | Older match (wraps around)              |
| `↓`            | Newer match (wraps around)              |
| `Backspace`    | Delete the last character               |
| `Ctrl+u`       | Clear the query                         |
| `Esc`          | Close the search                        |

Clicking inside the pane's content hands the keyboard back to the pane but
leaves the matches highlighted. Clicking the search bar gives the keyboard
back to the search. See [Search](features.md#search).

### Profile menu

Opened with the `[⌵]` button.

| Key        | Action                          |
| ---------- | ------------------------------- |
| `↑` / `↓`  | Move the selection (wraps)      |
| `Enter`    | Open a new tab with the profile |
| `Esc`      | Close the menu                  |

### Help screen

Any key or click closes it.

## Notes

- Unlike tmux, the prefix doesn't time out. After `^b`, the next key is always
  read as a command.
- Key encoding to the PTY honors the program's *application cursor* mode, so
  arrow keys work in vim, less, etc.
- When you add or change a binding in the code, update `HELP_LINES` and the
  button tooltips in `src/view.rs`, and this page (see
  [Contributing](contributing.md)).

# Features

## Tabs and splits

- `^b c` opens a new tab running a fresh shell. A [profile](workspace.md#profiles)
  (`[⌵]` menu) opens a tab with a preset shell, directory, environment and
  command.
- `^b %` / `^b "` split the focused pane in half. The new pane starts a fresh
  shell in TabMux's own working directory (not the directory of the pane
  you split) and takes the focus.
- Splits can be nested in any way. Each split has a ratio, kept between 10 %
  and 90 %, that you change by dragging the separator.
- A tab with no custom name shows its focused pane's title. Rename it with
  `^b r`, and set an empty name to go back to the pane title.

## Zoom

`^b z`, `[□]`, or a double-click on a pane title / sidebar row makes the
focused pane fill the whole main area. The other panes keep running and keep
their size. Focusing another pane, splitting, or pressing `^b z` again
restores the layout.

## Wide mode

`^b w` (`[↔]` → `[⇔]`) makes the pane's PTY **1000 columns** wide, whatever
the pane's real width. Long lines (logs, tables, `ps aux`, compiler output)
stay on one line instead of wrapping, and you view them through a window
that scrolls horizontally:

- A horizontal scrollbar shows up under the pane.
- Shift+wheel, horizontal wheel, or the scrollbar scroll by 8 columns.
- While you type, the view follows the cursor. Once you scroll by hand it
  stops following, until you type again.

Turning wide mode off resizes the PTY back to the pane's width.

## Multicursor

`^b m` (`[≡]` → `[▤]`) turns on multicursor for the **current tab**. Every key
and paste goes to all panes of the tab at once, which is handy for running
the same command on several hosts. Panes other than the focused one show a
fake cursor, and the title shows `[sync]`. Press `^b m` again to turn it off.
The status line confirms the change.

## Search

`^b /` (or `[⌕]`) opens a search bar in the focused pane's title row and looks
through the pane's **whole history** (scrollback and screen):

- **Smart case**: matching ignores case, unless the query has an uppercase
  letter.
- Results update as you type, and new output updates them too.
- All matches are highlighted, and the current one stands out. The bar shows
  the match count.
- `Enter` / `↑` / `[▲]` move to an older match, `↓` / `[▼]` to a newer one.
  Both wrap around. The first match selected is the most recent one.
- The pane scrolls so the current match sits about a third of the way down.
  In wide mode it also scrolls horizontally to the match.
- `Esc` / `[✕]` closes the search. Clicking the pane content hands the
  keyboard back to the shell and keeps the highlights.

Only one search is open at a time. Opening a search in another pane replaces
it.

## Prompt navigation

With [shell integration](shell-integration.md) active, TabMux records where
each shell prompt starts. You can then move through command output one
command at a time:

| Keys                      | Action                         |
| ------------------------- | ------------------------------ |
| `^b [` or `Ctrl+↑`        | Scroll to the previous prompt  |
| `^b ]` or `Ctrl+↓`        | Scroll to the next prompt      |

The prompt you jumped to is marked in the pane. Going past the newest prompt
scrolls back to the bottom. If no marks exist, the status line shows
`no prompt marks (shell integration off?)`.

`Ctrl+↑/↓` work only while the pane is on its normal screen, so full-screen
programs still get these keys.

## Bell and activity

When a pane in a **background** tab:

- rings the bell (BEL or visual bell), its tab label **blinks** every 500 ms
  until you switch to that tab;
- prints output, its tab label shows a `•` marker in an activity style.

Output that arrives right after a resize (300 ms) or at shell startup
(about 1 s) doesn't count as activity, so prompt redraws don't light up the
marker.

## Window titles

A program can set its pane's title with the standard OSC 0/2 sequence, as
most shells and editors do. The title appears in the pane's title row, in the
sidebar pane list, and as the tab name when the tab has no custom name.
Panes without a title show `shell N`.

## Scrollback

Each pane keeps **100 000 lines** of scrollback. You scroll with the wheel,
the scrollbar or `Shift+PgUp/PgDn`, and any typing brings the view back to
the bottom.

## Startup commands

A workspace pane or profile can have a `command`. TabMux types it into the
shell followed by Enter, as if you had typed it. When the command exits, the
shell is still there, so you can run it again with `↑ Enter`.

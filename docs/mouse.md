# Mouse

TabMux turns on mouse capture in the host terminal. A mouse event is either
handled by TabMux (buttons, borders, selection) or forwarded to the program
running in a pane.

## Forwarding to programs

If the program in a pane asks for mouse reporting (vim with `mouse=a`, htop,
tmux, …), clicks, drags, wheel and motion over its content go to that
program. TabMux sends them in the encoding the program asked for (SGR, UTF-8
or legacy X10), with Shift, Alt and Ctrl modifiers.

**Hold Shift to bypass the program.** A Shift-click or Shift-drag on the
content goes to TabMux instead, so you can select text even inside a
mouse-aware program.

## Focus and buttons

- Clicking a pane focuses it, and also switches to its tab.
- Buttons react to a left click, and hovering a button shows a tooltip. See
  [Getting started](getting-started.md#screen-tour) for the list.
- Clicking anywhere saves a tab rename in progress.

## Resizing

| Gesture                                    | Effect                                   |
| ------------------------------------------ | ---------------------------------------- |
| Drag a separator between panes             | Change the split ratio (kept within 10–90 %) |
| Drag the sidebar border                    | Change the sidebar width                 |
| `[<]` / `[>]` in the sidebar               | Sidebar 2 columns narrower / wider       |

The sidebar is at least 14 columns wide and at most half the screen.

## Tabs

| Gesture                              | Effect                     |
| ------------------------------------ | -------------------------- |
| Click a tab                          | Switch to it               |
| Double-click a tab                   | Rename it                  |
| Drag a tab along the sidebar / top bar | Reorder tabs             |
| `✎` / `×` on a sidebar tab row       | Rename / close the tab     |
| `‹` / `›` in the top bar             | Show hidden tabs           |
| Wheel over the sidebar               | Scroll the tab list (never switches tabs) |

## Moving panes (drag-and-drop)

Drag a pane by its **title row** or its **sidebar row**, then drop it on:

| Drop on                                     | Result                                                        |
| ------------------------------------------- | ------------------------------------------------------------- |
| Another pane's edge (left / right / top / bottom zone) | Placed next to that pane on that side, in any tab  |
| Another pane's center                       | The two panes swap places (works across tabs)                 |
| Another pane's sidebar row                  | Placed next to it. Within one tab, the two panes trade places in list order |
| A tab (sidebar row or top bar cell)         | Added to the side of that tab's whole layout                  |
| The `[+]` new-tab button                    | Moved into a new tab of its own                               |

The drop zone is highlighted while you drag. When a pane is dropped from the
sidebar, it keeps the side it had next to its old sibling (default: right).
A tab's only pane can't be dropped onto that same tab or onto `[+]`, since
nothing would change. If moving a pane empties its old tab, that tab closes.

Panes keep running while they move: TabMux never restarts the shell.

Clicking a sidebar pane row focuses that pane, and double-clicking it (or its
title row) toggles zoom.

## Scrolling

| Gesture                                 | Effect                                                       |
| --------------------------------------- | ------------------------------------------------------------ |
| Wheel over a pane                       | Scroll its history by 3 lines                                |
| Wheel over a full-screen program without mouse support | Sends 3 `↑` / `↓` key presses instead             |
| Shift+wheel over a pane in wide mode    | Scroll horizontally by 8 columns                             |
| Horizontal wheel / trackpad             | Scroll horizontally (wide mode)                              |
| Click / drag a scrollbar                | Jump / drag through history (or columns)                     |
| Scrollbar arrow cells                   | Step by 3 lines / 8 columns                                  |

## Selection, copy and paste

| Gesture                  | Effect                                                        |
| ------------------------ | ------------------------------------------------------------- |
| Left-drag in a pane      | Select text. It is copied when you release                    |
| Double-click             | Copy the word under the cursor (a run of printable ASCII with no spaces, so paths and URLs come out whole) |
| Triple-click             | Copy the line (up to its last non-blank character)            |
| Middle-click             | Paste the last copied text into the pane(s)                   |

Copying sends an **OSC 52** sequence to the host terminal, which puts the
text on the system clipboard (if the terminal supports it). TabMux also keeps
the text in its own clipboard for middle-click paste. The status line
confirms with `copied N chars`.

A selection is linear (from start to end, like a text editor) and limited to
the visible screen of one pane. Typing, scrolling or switching wide mode
clears it.

Pasting from the host terminal (bracketed paste) goes to the focused pane, or
to every pane of the tab in multicursor mode. If the program turned on
bracketed paste, the text is wrapped in `ESC[200~ … ESC[201~`. Otherwise
newlines are sent as carriage returns.

## Multi-click timing

Clicks at the same cell within 400 ms count as double, then triple. A fourth
click starts over at single.

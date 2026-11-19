#!/usr/bin/env python3
"""Render an ANSI screen capture (tmux capture-pane -e -p) to SVG.

Usage: ansi2svg.py COLS ROWS < capture.ansi > out.svg
"""

import re
import sys
from html import escape

CELL_W, CELL_H, FONT = 8.4, 17, 14
DEF_FG, DEF_BG = "#d0d0d0", "#1c1c1c"
BASE16 = [
    "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
    "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
]


def xterm256(n):
    if n < 16:
        return BASE16[n]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (steps[n // 36], steps[n // 6 % 6], steps[n % 6])
    v = 8 + (n - 232) * 10
    return "#%02x%02x%02x" % (v, v, v)


def sgr(params, st):
    it = iter(params or [0])
    for p in it:
        if p == 0:
            st.update(fg=None, bg=None, bold=False, dim=False, ital=False, ul=False, rev=False)
        elif p == 1:
            st["bold"] = True
        elif p == 2:
            st["dim"] = True
        elif p == 3:
            st["ital"] = True
        elif p == 4:
            st["ul"] = True
        elif p == 7:
            st["rev"] = True
        elif p == 22:
            st["bold"] = st["dim"] = False
        elif p == 23:
            st["ital"] = False
        elif p == 24:
            st["ul"] = False
        elif p == 27:
            st["rev"] = False
        elif 30 <= p <= 37:
            st["fg"] = BASE16[p - 30]
        elif 90 <= p <= 97:
            st["fg"] = BASE16[p - 90 + 8]
        elif 40 <= p <= 47:
            st["bg"] = BASE16[p - 40]
        elif 100 <= p <= 107:
            st["bg"] = BASE16[p - 100 + 8]
        elif p == 39:
            st["fg"] = None
        elif p == 49:
            st["bg"] = None
        elif p in (38, 48):
            key = "fg" if p == 38 else "bg"
            mode = next(it, None)
            if mode == 5:
                st[key] = xterm256(next(it, 0))
            elif mode == 2:
                st[key] = "#%02x%02x%02x" % (next(it, 0), next(it, 0), next(it, 0))


def parse(text, cols, rows):
    grid = [[(" ", DEF_FG, DEF_BG, False, False, False)] * cols for _ in range(rows)]
    st = dict(fg=None, bg=None, bold=False, dim=False, ital=False, ul=False, rev=False)
    for y, line in enumerate(text.split("\n")[:rows]):
        x = 0
        for tok in re.split(r"(\x1b\[[0-9;:]*m)", line):
            if tok.startswith("\x1b["):
                sgr([int(p) for p in re.split("[;:]", tok[2:-1]) if p], st)
                continue
            for ch in tok:
                if x >= cols:
                    break
                fg, bg = st["fg"] or DEF_FG, st["bg"] or DEF_BG
                if st["rev"]:
                    fg, bg = bg, fg
                if st["dim"]:
                    fg = "#808080"
                grid[y][x] = (ch, fg, bg, st["bold"], st["ital"], st["ul"])
                x += 1
    return grid


def render(grid, cols, rows):
    w, h = cols * CELL_W, rows * CELL_H
    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0f} {h}" '
        f'width="{w:.0f}" height="{h}" font-family="\'DejaVu Sans Mono\',Menlo,Consolas,monospace" '
        f'font-size="{FONT}">',
        f'<rect width="100%" height="100%" fill="{DEF_BG}"/>',
    ]
    # Background runs.
    for y, row in enumerate(grid):
        x = 0
        while x < cols:
            bg = row[x][2]
            end = x
            while end < cols and row[end][2] == bg:
                end += 1
            if bg != DEF_BG:
                out.append(
                    f'<rect x="{x * CELL_W:.1f}" y="{y * CELL_H}" '
                    f'width="{(end - x) * CELL_W + 0.3:.1f}" height="{CELL_H + 0.3}" fill="{bg}"/>'
                )
            x = end
    # Space-free glyph runs of one style. The per-glyph x list keeps
    # box-drawing aligned with any font; splitting at spaces keeps renderers
    # that ignore x lists close enough.
    for y, row in enumerate(grid):
        base = y * CELL_H + CELL_H * 0.78
        x = 0
        while x < cols:
            if row[x][0] == " ":
                x += 1
                continue
            style = row[x][1:2] + row[x][3:]
            end = x
            while end < cols and row[end][0] != " " and row[end][1:2] + row[end][3:] == style:
                end += 1
            fg, bold, ital, ul = style
            attrs = f'fill="{fg}"'
            if bold:
                attrs += ' font-weight="bold"'
            if ital:
                attrs += ' font-style="italic"'
            if ul:
                attrs += ' text-decoration="underline"'
            xs = " ".join(f"{i * CELL_W:.1f}" for i in range(x, end))
            text = escape("".join(c[0] for c in row[x:end]))
            out.append(f'<text x="{xs}" y="{base:.1f}" {attrs}>{text}</text>')
            x = end
    out.append("</svg>")
    return "\n".join(out) + "\n"


def main():
    cols, rows = int(sys.argv[1]), int(sys.argv[2])
    sys.stdout.write(render(parse(sys.stdin.read(), cols, rows), cols, rows))


if __name__ == "__main__":
    main()

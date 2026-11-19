use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use tui_term::widget::Cell as _;

use crate::app::{App, Drag, DropTarget, Mode};
use crate::layout::Dir;
use crate::tab::Tab;
use crate::view::{self, HELP_LINES, SidebarRow, Target};

const ACCENT_BG: Color = Color::Blue;
const IDLE_BG: Color = Color::Indexed(236);
const SIDEBAR_BG: Color = Color::Indexed(234);
const LINE_IDLE: Color = Color::DarkGray;
const LINE_ACTIVE: Color = Color::Yellow;
const ZOOM_FG: Color = Color::Green;
/// Separators, sidebar border and [▤] button of a tab in multicursor mode.
const SYNC_FG: Color = Color::Magenta;
/// Uniform background of the prompt reached with ^↑ / ^↓.
const HIGHLIGHT_BG: Color = Color::Indexed(24);
/// Area where a dragged pane will land.
const DROP_BG: Color = Color::Indexed(23);
const TOOLTIP_BG: Color = Color::Indexed(238);
/// Marker and color of a background tab that produced output.
const ACTIVITY_MARK: char = '•';
const ACTIVITY_FG: Color = Color::Yellow;
/// Half-period of a ringing tab's blink.
pub const BLINK: std::time::Duration = std::time::Duration::from_millis(500);

/// Highlight style for a ringing tab during the "on" half of the blink.
fn bell_style() -> Option<Style> {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    (ms / BLINK.as_millis()).is_multiple_of(2).then(|| {
        Style::default()
            .bg(Color::Red)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    })
}

fn shows_activity(tab: &Tab, active: bool) -> bool {
    tab.activity && !active && !tab.bell
}

/// Colors the label of a background tab with unseen output.
fn activity_style(tab: &Tab, active: bool, style: Style) -> Style {
    if shows_activity(tab, active) {
        style.fg(ACTIVITY_FG)
    } else {
        style
    }
}

/// Replaces the label's leading space with the activity marker.
fn activity_label(tab: &Tab, active: bool, label: String) -> String {
    match label.strip_prefix(' ') {
        Some(rest) if shows_activity(tab, active) => format!("{ACTIVITY_MARK}{rest}"),
        _ => label,
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let v = &app.view;
    let hovered = |r: Rect| app.hover.is_some_and(|p| r.contains(p));

    if v.sidebar.is_some() {
        draw_sidebar(f, app);
    }
    if let Some(tb) = v.topbar {
        draw_topbar(f, app, tb);
    }
    draw_status(f, app);
    if let Some(b) = v.border {
        let active = app.drag == Drag::Sidebar || hovered(b);
        let tab = app.tabs.get(app.active);
        let zoomed = tab.is_some_and(|t| t.zoom.is_some());
        let sync = tab.is_some_and(|t| t.sync);
        let fg = if active {
            LINE_ACTIVE
        } else if sync {
            SYNC_FG
        } else if zoomed {
            ZOOM_FG
        } else {
            LINE_IDLE
        };
        fill(f, b, "│", Style::default().fg(fg));
    }

    let tab = app.tabs.get(app.active);
    for pv in &v.panes {
        let Some(pane) = app.panes.get(&pv.id) else {
            continue;
        };
        let focused = tab.is_some_and(|t| t.focus == pv.id);

        if let Some(t) = pv.title {
            let style = if focused {
                Style::default()
                    .bg(ACCENT_BG)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(IDLE_BG).fg(Color::Gray)
            };
            let zoom = match (tab.is_some_and(|t| t.zoom == Some(pv.id)), pane.wide) {
                (true, true) => " [zoom] [1000 cols]",
                (true, false) => " [zoom]",
                (false, true) => " [1000 cols]",
                (false, false) => "",
            };
            let offset = if v.panes.first().is_some_and(|p| p.id == pv.id) && v.bar_hidden {
                3
            } else {
                0
            };
            let sync = if tab.is_some_and(|t| t.sync) {
                " [sync]"
            } else {
                ""
            };
            let text = format!("{:offset$} {}{zoom}{sync}", "", pane.title());
            f.render_widget(Paragraph::new(Line::styled(pad(&text, t.width), style)), t);

            if let Some(s) = app.search.as_ref().filter(|s| s.pane == pv.id) {
                let editing = matches!(app.mode, Mode::Search);
                let style = if editing {
                    Style::default().bg(Color::Magenta).fg(Color::White)
                } else {
                    Style::default().bg(IDLE_BG).fg(Color::White)
                };
                let count = match (s.current, s.matches.len()) {
                    (_, 0) if s.query.is_empty() => String::new(),
                    (_, 0) => "  no match".to_string(),
                    (Some(i), n) => format!("  {}/{n}", i + 1),
                    (None, n) => format!("  {n}"),
                };
                let caret = if editing { "▏" } else { "" };
                let text = format!(" ⌕ {}{caret}{count}", s.query);
                f.render_widget(Paragraph::new(Line::styled(pad(&text, t.width), style)), t);
            }
        }

        if let Some(bar) = pv.hscrollbar {
            let width = pv.content.width as usize;
            let max = (crate::pane::WIDE_COLS as usize).saturating_sub(width);
            let active = app.drag == Drag::HScrollbar(pv.id) || hovered(bar);
            draw_scrollbar(f, bar, pane.hscroll as usize, max, width, active, app.hover);
        }
        if let Some(bar) = pv.scrollbar {
            let (offset, max) = pane.scroll_info();
            let active = app.drag == Drag::Scrollbar(pv.id) || hovered(bar);
            draw_scrollbar(
                f,
                bar,
                max - offset,
                max,
                pv.content.height as usize,
                active,
                app.hover,
            );
        }

        let (scroll_off, scroll_max) = pane.scroll_info();
        // Before locking the parser: jumped_prompt() locks it too.
        let jumped = app.jumped_prompt(pv.id);
        let parser = pane.parser.lock().unwrap();
        let screen = parser.screen();
        let hscroll = pane.hscroll;
        render_screen(f.buffer_mut(), pv.content, screen, hscroll);

        if let Some(sel) = app
            .selection
            .as_ref()
            .filter(|s| s.pane == pv.id && !s.is_empty())
        {
            let buf = f.buffer_mut();
            for row in 0..pv.content.height {
                for col in 0..pv.content.width {
                    if sel.contains(row, col + hscroll) {
                        let p = Position::new(pv.content.x + col, pv.content.y + row);
                        if let Some(cell) = buf.cell_mut(p) {
                            invert(cell);
                        }
                    }
                }
            }
        }

        if let Some(s) = app.search.as_ref().filter(|s| s.pane == pv.id) {
            // History line shown on the first content row.
            let top = scroll_max - scroll_off;
            let current = s.current_match();
            let buf = f.buffer_mut();
            for m in &s.matches {
                let Some(row) = m.line.checked_sub(top) else {
                    continue;
                };
                if row >= pv.content.height as usize {
                    continue;
                }
                let style = if Some(*m) == current {
                    Style::default().bg(Color::LightRed).fg(Color::Black)
                } else {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                };
                for c in m.col..m.col + m.len {
                    let Some(x) = c.checked_sub(hscroll).filter(|x| *x < pv.content.width) else {
                        continue;
                    };
                    let p = Position::new(pv.content.x + x, pv.content.y + row as u16);
                    if let Some(cell) = buf.cell_mut(p) {
                        cell.set_style(style);
                    }
                }
            }
        }

        // Highlight the prompt reached with ^↑ / ^↓.
        if let Some(row) = jumped
            .and_then(|l| l.checked_sub(scroll_max - scroll_off))
            .filter(|&r| r < pv.content.height as usize)
        {
            let r = Rect {
                y: pv.content.y + row as u16,
                height: 1,
                ..pv.content
            };
            let buf = f.buffer_mut();
            for x in r.left()..r.right() {
                if let Some(cell) = buf.cell_mut(Position::new(x, r.y)) {
                    highlight(cell);
                }
            }
        }

        let typing = matches!(app.mode, Mode::Normal | Mode::Prefix);
        // Multicursor: other panes of the tab show a fake (inverted) cursor.
        let fake = !focused && tab.is_some_and(|t| t.sync);
        if (focused || fake) && typing && !screen.hide_cursor() && screen.scrollback() == 0 {
            let (row, col) = screen.cursor_position();
            if let Some(col) = col.checked_sub(hscroll)
                && row < pv.content.height
                && col < pv.content.width
            {
                let p = Position::new(pv.content.x + col, pv.content.y + row);
                if focused {
                    f.set_cursor_position(p);
                } else if let Some(cell) = f.buffer_mut().cell_mut(p) {
                    invert(cell);
                }
            }
        }
    }

    let sync = tab.is_some_and(|t| t.sync);
    for s in &v.seps {
        let active = app.drag == Drag::Split(s.path.clone()) || hovered(s.rect);
        let sym = match s.dir {
            Dir::Vertical => "│",
            Dir::Horizontal => "─",
        };
        fill(
            f,
            s.rect,
            sym,
            Style::default().fg(match (active, sync) {
                (true, _) => LINE_ACTIVE,
                (false, true) => SYNC_FG,
                (false, false) => LINE_IDLE,
            }),
        );
    }

    if let Drag::Pane {
        drop: Some(DropTarget::Pane(target, zone)),
        ..
    } = app.drag
        && let Some(pv) = v.pane(target)
    {
        let r = zone.rect(pv.rect);
        let buf = f.buffer_mut();
        for y in r.top()..r.bottom() {
            for x in r.left()..r.right() {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    cell.set_bg(DROP_BG);
                }
            }
        }
    }

    for b in &v.buttons {
        let style = if hovered(b.rect) {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            match b.target {
                Target::Quit | Target::ClosePane(_) | Target::TabClose(_) => {
                    Style::default().fg(Color::LightRed)
                }
                Target::Zoom(id) if app.tabs.iter().any(|t| t.zoom == Some(id)) => {
                    Style::default().fg(ZOOM_FG)
                }
                Target::Sync(_) if sync => Style::default().fg(SYNC_FG),
                _ => Style::default().fg(Color::LightYellow),
            }
        };
        f.buffer_mut()
            .set_string(b.rect.x, b.rect.y, &b.label, style);
    }

    if let Some(rect) = v.help {
        draw_help(f, rect);
    } else if let Some(rect) = v.menu {
        draw_menu(f, app, rect);
    } else if app.drag == Drag::None {
        draw_tooltip(f, app);
    }
}

/// One-line hint next to the hovered button.
fn draw_tooltip(f: &mut Frame, app: &App) {
    let Some(pos) = app.hover else { return };
    let Some(tip) = app
        .view
        .buttons
        .iter()
        .rev()
        .find(|b| b.rect.contains(pos))
        .and_then(|b| b.target.tooltip().map(|t| (b.rect, t)))
    else {
        return;
    };
    let (b, tip) = tip;
    let area = f.area();
    let text = format!(" {tip} ");
    let width = (view::unicode_width(&text) + 2).min(area.width);
    let height = 3;
    let y = if b.bottom() + height <= area.bottom() {
        b.bottom()
    } else if b.y >= area.y + height {
        b.y - height
    } else {
        return;
    };
    let x = b.x.min(area.right().saturating_sub(width)).max(area.x);
    let rect = Rect::new(x, y, width, height);
    let style = Style::default().bg(TOOLTIP_BG).fg(Color::White);
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(text).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style.fg(LINE_IDLE)),
        ),
        rect,
    );
}

fn draw_sidebar(f: &mut Frame, app: &App) {
    let v = &app.view;
    if let Some(s) = v.sidebar {
        f.render_widget(Block::default().style(Style::default().bg(SIDEBAR_BG)), s);
    }
    if let Some(h) = v.header {
        let style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let logo = vec![
            Line::styled(" ⢹⠁⢀⡀⣇ ⡗⠔⡇⡀⡀⡀⡀", style),
            Line::styled(" ⠸ ⠣⠇⠧⠃⠇ ⠇⠣⠇⠜⠄", style),
        ];
        f.render_widget(Paragraph::new(logo), h);
    }

    let focus = app.tabs.get(app.active).map(|t| t.focus);
    let accent = Style::default()
        .bg(ACCENT_BG)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let drop = match app.drag {
        Drag::Pane { drop, .. } => drop,
        _ => None,
    };
    let drop_style = Style::default().bg(DROP_BG).fg(Color::White);
    for &(r, row) in &v.rows {
        let hovered = app.hover.is_some_and(|p| row.contains(p));
        let (label, style) = match r {
            SidebarRow::Tab(i) => {
                let Some(tab) = app.tabs.get(i) else { break };
                let active = i == app.active;
                let name = match (&app.mode, active) {
                    (Mode::Rename(buf), true) => format!("{buf}▏"),
                    _ => app.tab_title(tab),
                };
                let split = tab.root.leaves().len() > 1;
                let style = if app.drag == Drag::Tab(i) {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else if drop == Some(DropTarget::Tab(i)) {
                    drop_style
                } else if active && split {
                    // The focused pane row carries the accent.
                    Style::default()
                        .bg(IDLE_BG)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if active {
                    accent
                } else if let Some(b) = bell_style().filter(|_| tab.bell) {
                    b
                } else if hovered {
                    Style::default().bg(IDLE_BG)
                } else {
                    Style::default()
                };
                let style = activity_style(tab, active, style);
                // Leave room for the ✎ × buttons.
                let label = truncate(&format!(" {} {name}", i + 1), row.width.saturating_sub(5));
                let label = activity_label(tab, active, label);
                (label, style)
            }
            SidebarRow::Pane { tab, id, last } => {
                let title = app.panes.get(&id).map(|p| p.title()).unwrap_or_default();
                let zoomed = app.tabs.get(tab).is_some_and(|t| t.zoom == Some(id));
                let branch = if last { '└' } else { '├' };
                let zoom = if zoomed { " [z]" } else { "" };
                let dragged = matches!(app.drag, Drag::Pane { id: d, .. } if d == id);
                let style = if app.drag == Drag::Tab(tab) || dragged {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else if matches!(drop, Some(DropTarget::Pane(t, _)) if t == id) {
                    drop_style
                } else if tab == app.active && focus == Some(id) {
                    accent
                } else if hovered {
                    Style::default().bg(IDLE_BG)
                } else {
                    Style::default().fg(Color::Gray)
                };
                // Leave room for the × button.
                let label = truncate(
                    &format!(" {branch} {title}{zoom}"),
                    row.width.saturating_sub(3),
                );
                (label, style)
            }
        };
        f.render_widget(
            Paragraph::new(Line::styled(pad(&label, row.width), style)),
            row,
        );
    }
    if let Some(bar) = v.tab_scrollbar {
        let cap = bar.height as usize;
        let max = v.rows_total.saturating_sub(cap);
        let active = app.drag == Drag::TabScrollbar || app.hover.is_some_and(|p| bar.contains(p));
        draw_scrollbar(f, bar, app.tab_scroll.min(max), max, cap, active, app.hover);
    }
}

/// Mode indicator or status message, in the sidebar or top bar.
fn draw_status(f: &mut Frame, app: &App) {
    let v = &app.view;
    if let Some(st) = v.status {
        let (text, style) = match &app.mode {
            Mode::Prefix => (
                "PREFIX".to_string(),
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Mode::Rename(_) => (
                "RENAME".to_string(),
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            Mode::Search => (
                "SEARCH".to_string(),
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            _ => {
                let sb_lines = app
                    .tabs
                    .get(app.active)
                    .and_then(|t| app.panes.get(&t.focus))
                    .map(|p| p.parser.lock().unwrap().screen().scrollback())
                    .unwrap_or(0);
                if let Some(s) = app.status() {
                    (s.to_string(), Style::default().fg(Color::Green))
                } else if sb_lines > 0 {
                    (
                        format!("scroll {sb_lines}"),
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    (
                        "^b ? help".to_string(),
                        Style::default().fg(Color::DarkGray),
                    )
                }
            }
        };
        f.render_widget(Paragraph::new(Line::styled(text, style)), st);
    }
}

/// Single row of tabs: ` 1 name × ` each, active one highlighted.
fn draw_topbar(f: &mut Frame, app: &App, tb: Rect) {
    let v = &app.view;
    f.render_widget(
        Paragraph::new(Line::styled(
            pad("", tb.width),
            Style::default().bg(IDLE_BG),
        )),
        tb,
    );
    let drop = match app.drag {
        Drag::Pane { drop, .. } => drop,
        _ => None,
    };
    for &(i, r) in &v.top_tabs {
        let Some(tab) = app.tabs.get(i) else { continue };
        let active = i == app.active;
        let name = match (&app.mode, active) {
            (Mode::Rename(buf), true) => format!("{buf}▏"),
            _ => app.tab_title(tab),
        };
        let hovered = app.hover.is_some_and(|p| r.contains(p));
        let style = if app.drag == Drag::Tab(i) {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else if drop == Some(DropTarget::Tab(i)) {
            Style::default().bg(DROP_BG).fg(Color::White)
        } else if active {
            Style::default()
                .bg(ACCENT_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if let Some(b) = bell_style().filter(|_| tab.bell) {
            b
        } else if hovered {
            Style::default().bg(Color::Indexed(238)).fg(Color::White)
        } else {
            Style::default().bg(IDLE_BG).fg(Color::Gray)
        };
        let style = activity_style(tab, active, style);
        let label = truncate(&view::top_tab_label(i, &name), r.width.saturating_sub(3));
        let label = activity_label(tab, active, label);
        f.render_widget(Paragraph::new(Line::styled(pad(&label, r.width), style)), r);
    }
}

/// Shell profile picker opened by the `[⌵]` button.
fn draw_menu(f: &mut Frame, app: &App, rect: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Profile ");
    f.render_widget(Clear, rect);
    f.render_widget(block, rect);
    let sel = app.menu.map_or(0, |m| m.sel);
    for &(i, r) in &app.view.menu_items {
        let Some((name, _)) = app.profiles.get(i) else {
            continue;
        };
        let style = if i == sel {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default()
        };
        let label = truncate(&format!(" {name}"), r.width);
        f.render_widget(Paragraph::new(Line::styled(pad(&label, r.width), style)), r);
    }
}

fn draw_help(f: &mut Frame, rect: Rect) {
    let line = |(k, d): &(&str, &'static str)| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!(" {k:<11}"), Style::default().fg(Color::Yellow)),
            Span::raw(*d),
        ])
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Ctrl-b + key · click to close ");
    let inner = block.inner(rect);
    f.render_widget(Clear, rect);
    f.render_widget(block, rect);
    if inner.width <= view::HELP_COL_W {
        f.render_widget(
            Paragraph::new(HELP_LINES.iter().map(line).collect::<Vec<_>>()),
            inner,
        );
        return;
    }
    let split = view::help_split();
    let (left, right) = HELP_LINES.split_at(split);
    let right = match right.first() {
        Some(("", "")) => &right[1..],
        _ => right,
    };
    let col = Rect {
        width: view::HELP_COL_W,
        ..inner
    };
    f.render_widget(
        Paragraph::new(left.iter().map(line).collect::<Vec<_>>()),
        col,
    );
    let sep = Rect {
        x: col.x + col.width,
        width: 1,
        ..inner
    };
    f.render_widget(
        Paragraph::new(vec![Line::raw("│"); sep.height as usize])
            .style(Style::default().fg(Color::DarkGray)),
        sep,
    );
    let col2 = Rect {
        x: sep.x + 1,
        width: inner.width.saturating_sub(view::HELP_COL_W + 1),
        ..inner
    };
    f.render_widget(
        Paragraph::new(right.iter().map(line).collect::<Vec<_>>()),
        col2,
    );
}

/// Scrollbar with arrows at both ends: vertical (▲/▼), or horizontal (◀/▶)
/// when `bar` is a single row. `pos` is in `0..=max` (0 = top/left),
/// `viewport` is the number of visible lines or columns.
fn draw_scrollbar(
    f: &mut Frame,
    bar: Rect,
    pos: usize,
    max: usize,
    viewport: usize,
    active: bool,
    hover: Option<Position>,
) {
    let horizontal = bar.height == 1 && bar.width > 1;
    let len_cells = if horizontal { bar.width } else { bar.height };
    if len_cells < 3 {
        return;
    }
    // Screen position of the i-th cell along the bar.
    let at = |i: u16| {
        if horizontal {
            Position::new(bar.x + i, bar.y)
        } else {
            Position::new(bar.x, bar.y + i)
        }
    };
    let arrow = |p: Position| {
        let hot = hover == Some(p);
        Style::default().fg(if hot { LINE_ACTIVE } else { Color::Gray })
    };
    let (first, last) = if horizontal {
        ("◀", "▶")
    } else {
        ("▲", "▼")
    };
    let buf = f.buffer_mut();
    let (p0, p1) = (at(0), at(len_cells - 1));
    buf.set_string(p0.x, p0.y, first, arrow(p0));
    buf.set_string(p1.x, p1.y, last, arrow(p1));

    let track = (len_cells - 2) as usize;
    let (start, len) = if max == 0 {
        (0, 0)
    } else {
        let len = ((track * viewport) as f32 / (max + viewport) as f32)
            .round()
            .clamp(1.0, track as f32) as usize;
        let start = (((track - len) * pos) as f32 / max as f32).round() as usize;
        (start, len)
    };
    let thumb = Style::default().fg(if active { LINE_ACTIVE } else { Color::Gray });
    let rail = Style::default().fg(LINE_IDLE);
    let (thumb_sym, rail_sym) = if horizontal {
        ("▬", "─")
    } else {
        ("█", "│")
    };
    for i in 0..track {
        let p = at(1 + i as u16);
        if (start..start + len).contains(&i) {
            buf.set_string(p.x, p.y, thumb_sym, thumb);
        } else {
            buf.set_string(p.x, p.y, rail_sym, rail);
        }
    }
}

/// Render a vt100 screen into `area`, starting at screen column `col_off`.
fn render_screen(buf: &mut Buffer, area: Rect, screen: &vt100::Screen, col_off: u16) {
    for row in 0..area.height {
        for col in 0..area.width {
            let Some(cell) = buf.cell_mut(Position::new(area.x + col, area.y + row)) else {
                continue;
            };
            cell.reset();
            if let Some(sc) = screen.cell(row, col + col_off) {
                sc.apply(cell);
            }
        }
    }
}

/// Paint a cell with the uniform highlight colors, whatever its own colors.
fn highlight(cell: &mut ratatui::buffer::Cell) {
    cell.modifier.remove(Modifier::REVERSED);
    cell.set_style(Style::default().bg(HIGHLIGHT_BG).fg(Color::White));
}

/// Swap a cell's colors. Toggles rather than adds REVERSED so that cells
/// already in inverse video flip too.
fn invert(cell: &mut ratatui::buffer::Cell) {
    cell.modifier.toggle(Modifier::REVERSED);
}

fn fill(f: &mut Frame, r: Rect, sym: &str, style: Style) {
    let buf = f.buffer_mut();
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_symbol(sym).set_style(style);
            }
        }
    }
}

fn pad(s: &str, width: u16) -> String {
    format!("{s:<w$}", w = width as usize)
}

fn truncate(s: &str, width: u16) -> String {
    s.chars().take(width as usize).collect()
}

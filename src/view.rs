use ratatui::layout::{Position, Rect};
use std::borrow::Cow;

use crate::app::{App, Mode};
use crate::layout::{PaneId, Path, Separator};
use crate::tab::Tab;

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    NewTab,
    Help,
    CloseHelp,
    HideSidebar,
    ShowSidebar,
    Quit,
    SidebarNarrow,
    SidebarWiden,
    SidebarBorder,
    /// Switch between the sidebar and the top tab bar.
    ToggleTabBar,
    /// Show / hide pane title rows.
    ToggleTitles,
    Tab(usize),
    TabRename(usize),
    TabClose(usize),
    PaneRow(PaneId),
    PaneTitle(PaneId),
    SplitV(PaneId),
    SplitH(PaneId),
    Zoom(PaneId),
    ClosePane(PaneId),
    Separator(Path),
    PaneContent(PaneId),
    Scrollbar(PaneId),
    HScrollbar(PaneId),
    Wide(PaneId),
    /// Toggle multicursor on the pane's tab.
    Sync(PaneId),
    Search(PaneId),
    SearchBar,
    SearchPrev,
    SearchNext,
    SearchClose,
    TabScrollbar,
    /// Open the shell profile menu.
    ProfileMenu,
    /// New tab with this shell profile (menu item).
    Profile(usize),
    /// Click outside the open profile menu.
    CloseMenu,
}

/// Label of the profile menu button.
pub const PROFILE_BUTTON: &str = "[⌵]";

impl Target {
    /// Hover text for buttons acting on this target.
    pub fn tooltip(&self) -> Option<&'static str> {
        Some(match self {
            Target::NewTab => "New tab (^b c)",
            Target::ProfileMenu => "New tab with profile",
            Target::Help => "Help (^b ?)",
            Target::ToggleTabBar => "Sidebar / top bar (^b t)",
            Target::HideSidebar => "Hide tab list (^b s)",
            Target::ShowSidebar => "Show tab list (^b s)",
            Target::Quit => "Quit (^b q)",
            Target::SidebarNarrow => "Shrink sidebar (^b <)",
            Target::SidebarWiden => "Grow sidebar (^b >)",
            Target::ToggleTitles => "Show / hide pane titles (^b T)",
            Target::TabRename(_) => "Rename tab (^b r)",
            Target::TabClose(_) => "Close tab (^b &)",
            Target::ClosePane(_) => "Close pane (^b x)",
            Target::SplitV(_) => "Split side by side (^b %)",
            Target::SplitH(_) => "Split top / bottom (^b \")",
            Target::Zoom(_) => "Zoom pane (^b z)",
            Target::Wide(_) => "Wide mode (^b w)",
            Target::Sync(_) => "Multicursor (^b m)",
            Target::Search(_) => "Search history (^b /)",
            Target::SearchPrev => "Older match",
            Target::SearchNext => "Newer match",
            Target::SearchClose => "Close search",
            _ => return None,
        })
    }
}

pub struct Button {
    pub rect: Rect,
    pub label: Cow<'static, str>,
    pub target: Target,
}

/// Where the tab list is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabBar {
    /// Vertical list on the left, with the panes of split tabs.
    #[default]
    Side,
    /// Single row of tabs at the top.
    Top,
}

impl TabBar {
    pub fn toggle(self) -> Self {
        match self {
            TabBar::Side => TabBar::Top,
            TabBar::Top => TabBar::Side,
        }
    }
}

/// Longest tab name shown in the top bar.
const TOP_TAB_MAX: usize = 20;
/// Columns reserved for the status text in the top bar.
const TOP_STATUS: u16 = 12;

/// One line of the sidebar list: a tab, or one of its panes when split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarRow {
    Tab(usize),
    Pane { tab: usize, id: PaneId, last: bool },
}

impl SidebarRow {
    pub fn tab(self) -> usize {
        match self {
            SidebarRow::Tab(i) | SidebarRow::Pane { tab: i, .. } => i,
        }
    }
}

/// Sidebar lines: each tab, followed by its panes if it has more than one.
pub fn sidebar_rows(tabs: &[Tab]) -> Vec<SidebarRow> {
    let mut out = Vec::new();
    for (i, t) in tabs.iter().enumerate() {
        out.push(SidebarRow::Tab(i));
        let leaves = t.root.leaves();
        if leaves.len() > 1 {
            let n = leaves.len();
            out.extend(
                leaves
                    .into_iter()
                    .enumerate()
                    .map(|(k, id)| SidebarRow::Pane {
                        tab: i,
                        id,
                        last: k + 1 == n,
                    }),
            );
        }
    }
    out
}

pub struct PaneView {
    pub id: PaneId,
    /// Whole pane: title, content and scrollbars.
    pub rect: Rect,
    pub title: Option<Rect>,
    pub content: Rect,
    pub scrollbar: Option<Rect>,
    /// Horizontal scrollbar, only in wide mode.
    pub hscrollbar: Option<Rect>,
    /// Close button above the scrollbar, when there is no title row.
    pub close: Option<Rect>,
}

#[derive(Default)]
pub struct View {
    pub sidebar: Option<Rect>,
    pub border: Option<Rect>,
    /// Top tab bar, in `TabBar::Top` mode.
    pub topbar: Option<Rect>,
    /// Tabs shown in the top bar.
    pub top_tabs: Vec<(usize, Rect)>,
    /// Tab list hidden: the first pane title carries the show button.
    pub bar_hidden: bool,
    pub main: Rect,
    pub header: Option<Rect>,
    /// Where the status text goes.
    pub status: Option<Rect>,
    /// Visible sidebar rows.
    pub rows: Vec<(SidebarRow, Rect)>,
    /// Total number of sidebar rows, visible or not.
    pub rows_total: usize,
    pub tab_scrollbar: Option<Rect>,
    pub panes: Vec<PaneView>,
    pub seps: Vec<Separator>,
    pub buttons: Vec<Button>,
    pub help: Option<Rect>,
    /// Open profile menu box and its item rows (profile index, rect).
    pub menu: Option<Rect>,
    pub menu_items: Vec<(usize, Rect)>,
    hits: Vec<(Rect, Target)>,
}

pub const HELP_LINES: &[(&str, &str)] = &[
    ("c", "new tab"),
    ("& ", "close tab"),
    ("n / p", "next / previous tab"),
    ("1-9", "go to tab"),
    ("r", "rename tab"),
    ("% / |", "split side by side"),
    ("\" / -", "split top / bottom"),
    ("o", "next pane"),
    ("^PgUp/PgDn", "prev / next tab or pane (no prefix)"),
    ("⇧PgUp/PgDn", "scroll pane by page (no prefix)"),
    ("←↑↓→ hjkl", "focus pane in direction"),
    ("z", "zoom pane"),
    ("w", "wide mode (1000 columns)"),
    ("m", "multicursor: type in all panes of tab"),
    ("/", "search history (↑/Enter older, ↓ newer)"),
    ("[ / ]", "previous / next command (also ^↑ / ^↓)"),
    ("x", "close pane"),
    ("< / >", "shrink / grow sidebar"),
    ("s", "show / hide tab list"),
    ("t", "sidebar / top tab bar"),
    ("T", "show / hide pane titles"),
    ("q", "quit"),
    ("^b", "send literal ^b"),
    ("", ""),
    ("mouse", "buttons, click to focus, drag borders"),
    ("", "drag tabs to reorder, dbl-click rename"),
    ("", "drag pane title / sidebar row onto a pane,"),
    ("", "  a sidebar tab / pane, or + (new tab)"),
    ("", "click sidebar pane to focus, dbl-click zoom"),
    ("", "drag to select+copy, middle-click paste"),
    ("", "dbl-click copies word, triple-click line"),
    ("", "shift+mouse bypasses apps"),
];

/// Inner width of one help column.
pub const HELP_COL_W: u16 = 50;

/// Index where the second help column starts in two-column layout:
/// roughly half, never splitting a continuation line from its entry.
pub fn help_split() -> usize {
    let mut i = HELP_LINES.len().div_ceil(2);
    while i > 0 && HELP_LINES[i].0.is_empty() && !HELP_LINES[i].1.is_empty() {
        i -= 1;
    }
    i
}

pub struct Split {
    pub sidebar: Option<Rect>,
    pub border: Option<Rect>,
    pub topbar: Option<Rect>,
    pub main: Rect,
}

/// Split the screen into the tab list (sidebar + border, or top bar) and main.
pub fn split_screen(size: Rect, sidebar_width: u16, visible: bool, bar: TabBar) -> Split {
    let none = Split {
        sidebar: None,
        border: None,
        topbar: None,
        main: size,
    };
    if !visible {
        return none;
    }
    match bar {
        TabBar::Top if size.height >= 2 => Split {
            topbar: Some(Rect { height: 1, ..size }),
            main: Rect {
                y: size.y + 1,
                height: size.height - 1,
                ..size
            },
            ..none
        },
        TabBar::Side if size.width >= 3 => {
            let sw = sidebar_width.min(size.width - 2);
            let border = Rect {
                x: size.x + sw,
                width: 1,
                ..size
            };
            Split {
                sidebar: Some(Rect { width: sw, ..size }),
                border: Some(border),
                topbar: None,
                main: Rect {
                    x: border.x + 1,
                    width: size.width - sw - 1,
                    ..size
                },
            }
        }
        _ => none,
    }
}

/// Label of a tab in the top bar, without the close button.
pub fn top_tab_label(index: usize, name: &str) -> String {
    let name: String = name.chars().take(TOP_TAB_MAX).collect();
    format!(" {} {name}", index + 1)
}

/// First tab to show in the top bar so that `active` fits in `avail` columns,
/// given each tab's width.
fn top_first(widths: &[u16], active: usize, avail: u16) -> usize {
    let mut first = active.min(widths.len().saturating_sub(1));
    let mut used = widths.get(first).copied().unwrap_or(0);
    while first > 0 && used + widths[first - 1] <= avail {
        first -= 1;
        used += widths[first];
    }
    first
}

fn pane_view(id: PaneId, r: Rect, wide: bool, show_title: bool) -> PaneView {
    let (title, body) = if show_title && r.height >= 2 {
        (
            Some(Rect { height: 1, ..r }),
            Rect {
                y: r.y + 1,
                height: r.height - 1,
                ..r
            },
        )
    } else {
        (None, r)
    };
    // Reserve the rightmost column for the scrollbar when there is room.
    let (content, scrollbar) = if body.width >= 3 && body.height >= 3 {
        (
            Rect {
                width: body.width - 1,
                ..body
            },
            Some(Rect {
                x: body.right() - 1,
                width: 1,
                ..body
            }),
        )
    } else {
        (body, None)
    };
    // No title row: the top scrollbar cell becomes a close button.
    let min_bar = if wide { 5 } else { 4 };
    let (scrollbar, close) = match scrollbar {
        Some(s) if title.is_none() && s.height >= min_bar => (
            Some(Rect {
                y: s.y + 1,
                height: s.height - 1,
                ..s
            }),
            Some(Rect { height: 1, ..s }),
        ),
        _ => (scrollbar, None),
    };
    // Wide mode: bottom row becomes a horizontal scrollbar.
    let (content, scrollbar, hscrollbar) = if wide && content.height >= 3 && content.width >= 3 {
        let content = Rect {
            height: content.height - 1,
            ..content
        };
        let hbar = Rect {
            y: content.bottom(),
            height: 1,
            ..content
        };
        let scrollbar = scrollbar.map(|s| Rect {
            height: s.height - 1,
            ..s
        });
        (content, scrollbar, Some(hbar))
    } else {
        (content, scrollbar, None)
    };
    PaneView {
        id,
        rect: r,
        title,
        content,
        scrollbar,
        hscrollbar,
        close,
    }
}

/// Swap axes so that horizontal bars can reuse vertical-bar math.
pub fn transpose(r: Rect) -> Rect {
    Rect::new(r.y, r.x, r.height, r.width)
}

/// Rows taken by the sidebar header (logo).
pub const HEADER_ROWS: u16 = 2;

/// Number of pane buttons shown in the sidebar / top bar.
const PANE_BUTTONS: u16 = 7;
/// Pane buttons in the tab list: no close, the pane has its own `x`.
const BAR_BUTTONS: u16 = PANE_BUTTONS - 1;

/// Rows taken by the pane buttons under the sidebar header (titles hidden).
pub fn pane_bar_rows(width: u16) -> u16 {
    if width >= 3 * BAR_BUTTONS.div_ceil(2) {
        2
    } else {
        0
    }
}

/// Number of rows the sidebar list can display. `pane_bar`: the pane buttons
/// sit under the header (titles hidden).
pub fn tab_capacity(
    size: Rect,
    sidebar_width: u16,
    visible: bool,
    bar: TabBar,
    pane_bar: bool,
) -> usize {
    match split_screen(size, sidebar_width, visible, bar).sidebar {
        Some(sb) => {
            let footer = if sb.height >= 3 { 1 } else { 0 };
            let pane_rows = if pane_bar { pane_bar_rows(sb.width) } else { 0 };
            sb.height.saturating_sub(HEADER_ROWS + footer + pane_rows) as usize
        }
        None => 0,
    }
}

/// Buttons for the focused pane of `tab` in the tab list, without close.
/// Order: zoom, splits, then wide, sync, search (two rows of three).
fn bar_buttons(app: &App, tab: &Tab) -> impl Iterator<Item = (&'static str, Target)> {
    let [zoom, split_v, split_h, wide, sync, search, _close] = pane_buttons(app, tab, tab.focus);
    [zoom, split_v, split_h, wide, sync, search].into_iter()
}

/// Buttons acting on pane `id` of `tab`.
fn pane_buttons(
    app: &App,
    tab: &Tab,
    id: PaneId,
) -> [(&'static str, Target); PANE_BUTTONS as usize] {
    let zoom = if tab.zoom == Some(id) {
        "[▣]"
    } else {
        "[□]"
    };
    let wide = if app.is_wide(id) { "[⇔]" } else { "[↔]" };
    let sync = if tab.sync { "[▤]" } else { "[≡]" };
    [
        (zoom, Target::Zoom(id)),
        ("[│]", Target::SplitV(id)),
        ("[─]", Target::SplitH(id)),
        (wide, Target::Wide(id)),
        (sync, Target::Sync(id)),
        ("[⌕]", Target::Search(id)),
        ("[✕]", Target::ClosePane(id)),
    ]
}

/// Map a row on a scrollbar (with arrow cells at both ends) to a position in `0..=max`.
pub fn bar_pos(bar: Rect, y: u16, max: usize) -> usize {
    let track = bar.height.saturating_sub(2).max(1);
    let rel = y.saturating_sub(bar.y + 1).min(track - 1);
    if track <= 1 {
        return 0;
    }
    ((rel as f32 / (track - 1) as f32) * max as f32).round() as usize
}

/// Pane views of a tab inside `main`. With `honor_zoom`, a zoomed pane fills `main`.
/// `wide` tells which panes are in wide mode, `title` which have a title row.
pub fn pane_views(
    tab: &Tab,
    main: Rect,
    honor_zoom: bool,
    wide: impl Fn(PaneId) -> bool,
    title: impl Fn(PaneId) -> bool,
) -> (Vec<PaneView>, Vec<Separator>) {
    if let (true, Some(z)) = (honor_zoom, tab.zoom) {
        return (vec![pane_view(z, main, wide(z), title(z))], Vec::new());
    }
    let (panes, seps) = tab.root.compute(main);
    (
        panes
            .into_iter()
            .map(|(id, r)| pane_view(id, r, wide(id), title(id)))
            .collect(),
        seps,
    )
}

impl View {
    pub fn build(app: &App) -> Self {
        let mut v = View::default();
        let Split {
            sidebar,
            border,
            topbar,
            main,
        } = split_screen(
            app.size,
            app.sidebar_width,
            app.sidebar_visible,
            app.tab_bar,
        );
        v.sidebar = sidebar;
        v.border = border;
        v.topbar = topbar;
        v.main = main;
        v.bar_hidden = sidebar.is_none() && topbar.is_none();

        if let Some(sb) = sidebar {
            v.build_sidebar(app, sb);
        }
        if let Some(tb) = topbar {
            v.build_topbar(app, tb);
        }
        if let Some(b) = border {
            v.hits.push((b, Target::SidebarBorder));
        }

        if let Some(tab) = app.tabs.get(app.active) {
            let (panes, seps) = pane_views(
                tab,
                main,
                true,
                |id| app.is_wide(id),
                |id| app.shows_title(id),
            );
            for (i, pv) in panes.iter().enumerate() {
                v.hits.push((pv.content, Target::PaneContent(pv.id)));
                if let Some(sb) = pv.scrollbar {
                    v.hits.push((sb, Target::Scrollbar(pv.id)));
                }
                if let Some(hb) = pv.hscrollbar {
                    v.hits.push((hb, Target::HScrollbar(pv.id)));
                }
                if let Some(c) = pv.close {
                    v.button(c.x, c.y, "x", Target::ClosePane(pv.id));
                }
                let searching = app.search.as_ref().is_some_and(|s| s.pane == pv.id);
                if let (Some(t), true) = (pv.title, searching) {
                    v.hits.push((t, Target::SearchBar));
                    if t.width > 10 {
                        let mut x = t.right() - 9;
                        for (label, target) in [
                            ("[▲]", Target::SearchPrev),
                            ("[▼]", Target::SearchNext),
                            ("[✕]", Target::SearchClose),
                        ] {
                            v.button(x, t.y, label, target);
                            x += 3;
                        }
                    }
                } else if let Some(t) = pv.title {
                    v.hits.push((t, Target::PaneTitle(pv.id)));
                    // Titles hidden: pane buttons live in the tab list.
                    if !app.titles_shown() {
                        continue;
                    }
                    let mut left = t.x;
                    if i == 0 && v.bar_hidden {
                        v.button(left, t.y, "[▶]", Target::ShowSidebar);
                        left += 3;
                    }
                    let btns = pane_buttons(app, tab, pv.id);
                    let need = 3 * btns.len() as u16;
                    if t.right().saturating_sub(left) > need {
                        let mut x = t.right() - need;
                        for (label, target) in btns {
                            v.button(x, t.y, label, target);
                            x += 3;
                        }
                    }
                }
            }
            for s in &seps {
                v.hits.push((s.rect, Target::Separator(s.path.clone())));
            }
            v.panes = panes;
            v.seps = seps;
        }

        if matches!(app.mode, Mode::Help) {
            let area = app.size;
            let two = HELP_LINES.len() as u16 + 2 > area.height;
            let rows = if two { help_split() } else { HELP_LINES.len() };
            let w = if two {
                HELP_COL_W * 2 + 3
            } else {
                HELP_COL_W + 2
            }
            .min(area.width);
            let h = (rows as u16 + 2).min(area.height);
            v.help = Some(Rect {
                x: area.x + (area.width - w) / 2,
                y: area.y + (area.height - h) / 2,
                width: w,
                height: h,
            });
            v.hits.push((area, Target::CloseHelp));
        }
        if let Some(menu) = &app.menu {
            v.build_menu(app, menu.anchor);
        }
        v
    }

    /// Profile menu below (or above) `anchor`, kept inside the screen.
    fn build_menu(&mut self, app: &App, anchor: Position) {
        let area = app.size;
        let names = app.profiles.iter().map(|(n, _)| unicode_width(n));
        let w = (names.max().unwrap_or(0) + 4).min(area.width);
        let h = (app.profiles.len() as u16 + 2).min(area.height);
        if w < 3 || h < 3 {
            return;
        }
        let below = anchor.y + 1;
        let y = if below + h <= area.bottom() {
            below
        } else {
            anchor.y.saturating_sub(h).max(area.y)
        };
        let x = anchor.x.min(area.right() - w);
        let rect = Rect::new(x, y, w, h);
        self.menu = Some(rect);
        self.hits.push((area, Target::CloseMenu));
        self.hits.push((rect, Target::CloseMenu));
        for i in 0..(h - 2) as usize {
            let row = Rect::new(x + 1, y + 1 + i as u16, w - 2, 1);
            self.menu_items.push((i, row));
            self.hits.push((row, Target::Profile(i)));
        }
    }

    fn build_sidebar(&mut self, app: &App, sb: Rect) {
        let header = Rect {
            height: HEADER_ROWS.min(sb.height),
            ..sb
        };
        self.header = Some(header);
        let btns = [
            ("[▔]", Target::ToggleTabBar),
            ("[◀]", Target::HideSidebar),
            ("[✕]", Target::Quit),
            ("[+]", Target::NewTab),
            ("[?]", Target::Help),
            ("[▭]", Target::ToggleTitles),
        ];
        // Split the buttons over the header rows, as a left-aligned block
        // flush with the right edge.
        let rows = header.height.max(1) as usize;
        let per_row = btns.len().div_ceil(rows);
        if sb.width >= 3 * per_row as u16 {
            let left = sb.right() - 3 * per_row as u16;
            for (r, chunk) in btns.chunks(per_row).enumerate() {
                let mut x = left;
                for (label, target) in chunk {
                    self.button(x, sb.y + r as u16, *label, target.clone());
                    x += 3;
                }
            }
        }

        if sb.height >= 3 {
            let status = Rect {
                y: sb.bottom() - 1,
                height: 1,
                ..sb
            };
            self.status = Some(Rect {
                x: status.x + 7,
                width: status.width.saturating_sub(7),
                ..status
            });
            if sb.width >= 6 {
                self.button(sb.x, status.y, "[<]", Target::SidebarNarrow);
                self.button(sb.x + 3, status.y, "[>]", Target::SidebarWiden);
            }
        }

        let last = if sb.height >= 3 {
            sb.bottom() - 1
        } else {
            sb.bottom()
        };
        let bar_rows = if app.titles_visible {
            0
        } else {
            pane_bar_rows(sb.width).min(last.saturating_sub(sb.y + HEADER_ROWS))
        };
        if let (true, Some(tab)) = (bar_rows > 0, app.tabs.get(app.active)) {
            let per_row = (BAR_BUTTONS as usize).div_ceil(bar_rows as usize);
            // Right-aligned.
            let x0 = sb.right() - 3 * per_row as u16;
            for (k, (label, target)) in bar_buttons(app, tab).enumerate() {
                let x = x0 + 3 * (k % per_row) as u16;
                self.button(x, sb.y + HEADER_ROWS + (k / per_row) as u16, label, target);
            }
        }
        let first = (sb.y + HEADER_ROWS + bar_rows).min(last);
        let cap = (last - first) as usize;
        let rows = sidebar_rows(&app.tabs);
        // One extra virtual row for the new-tab button under the last tab.
        self.rows_total = rows.len() + 1;
        let offset = app.tab_scroll.min(self.rows_total.saturating_sub(cap));
        let mut y_end = first;
        for (&r, y) in rows[offset.min(rows.len())..].iter().zip(first..last) {
            y_end = y + 1;
            let rect = Rect { y, height: 1, ..sb };
            self.rows.push((r, rect));
            match r {
                SidebarRow::Tab(i) => {
                    self.hits.push((rect, Target::Tab(i)));
                    if sb.width >= 8 {
                        self.button(rect.right() - 4, y, "✎", Target::TabRename(i));
                        self.button(rect.right() - 2, y, "×", Target::TabClose(i));
                    }
                }
                SidebarRow::Pane { id, .. } => {
                    self.hits.push((rect, Target::PaneRow(id)));
                    if sb.width >= 8 {
                        self.button(rect.right() - 2, y, "×", Target::ClosePane(id));
                    }
                }
            }
        }
        let has_bar = self.rows_total > cap && cap >= 3;
        // New-tab button under the last tab, spanning the sidebar width
        // (minus the scrollbar column).
        let mut w = sb.width - u16::from(has_bar);
        if y_end < last && !app.profiles.is_empty() && w >= 6 {
            w -= 3;
            self.button(sb.x + w, y_end, PROFILE_BUTTON, Target::ProfileMenu);
        }
        if y_end < last && w >= 3 {
            let inner = (w - 3) as usize;
            let label = format!(
                "[{}+{}]",
                " ".repeat(inner / 2),
                " ".repeat(inner - inner / 2)
            );
            self.button(sb.x, y_end, label, Target::NewTab);
        }
        if has_bar {
            let bar = Rect {
                x: sb.right() - 1,
                y: first,
                width: 1,
                height: cap as u16,
            };
            self.tab_scrollbar = Some(bar);
            self.hits.push((bar, Target::TabScrollbar));
        }
    }

    fn build_topbar(&mut self, app: &App, tb: Rect) {
        let mut btns = vec![
            ("[▭]", Target::ToggleTitles),
            ("[?]", Target::Help),
            ("[✕]", Target::Quit),
        ];
        let mut left = tb.x;
        let mut right = tb.right();
        if let (false, Some(tab)) = (app.titles_visible, app.tabs.get(app.active)) {
            let pane: Vec<_> = bar_buttons(app, tab).collect();
            if tb.width >= 3 * (btns.len() + pane.len()) as u16 + 3 + 30 {
                btns.splice(0..0, pane);
            }
        }
        let need = 3 * btns.len() as u16;
        if tb.width >= need + 3 + 10 {
            self.button(left, tb.y, "[▏]", Target::ToggleTabBar);
            left += 3;
            right -= need;
            let mut x = right;
            for (label, target) in btns {
                self.button(x, tb.y, label, target);
                x += 3;
            }
        }
        if right - left >= TOP_STATUS + 20 {
            right -= TOP_STATUS;
            self.status = Some(Rect {
                x: right,
                width: TOP_STATUS,
                ..tb
            });
        }

        // Each tab: label, then " × " close button.
        let widths: Vec<u16> = app
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| unicode_width(&top_tab_label(i, &app.tab_title(t))) + 3)
            .collect();
        // Leave room for the ‹ › overflow markers and the [+] / [⌵] buttons.
        let new_w = if app.profiles.is_empty() { 3 } else { 6 };
        let avail = (right - left).saturating_sub(2 + new_w);
        let first = top_first(&widths, app.active, avail);
        let mut x = left + 1;
        let mut shown = first;
        for (i, &w) in widths.iter().enumerate().skip(first) {
            if x + w > left + 1 + avail {
                break;
            }
            let rect = Rect::new(x, tb.y, w, 1);
            self.top_tabs.push((i, rect));
            self.hits.push((rect, Target::Tab(i)));
            self.button(rect.right() - 2, tb.y, "×", Target::TabClose(i));
            x += w;
            shown = i + 1;
        }
        if first > 0 {
            self.button(left, tb.y, "‹", Target::Tab(first - 1));
        }
        if x + 3 <= right {
            self.button(x, tb.y, "[+]", Target::NewTab);
            x += 3;
            if !app.profiles.is_empty() && x + 3 <= right {
                self.button(x, tb.y, PROFILE_BUTTON, Target::ProfileMenu);
                x += 3;
            }
        }
        if shown < widths.len() {
            self.button(x, tb.y, "›", Target::Tab(shown));
        }
    }

    fn button(&mut self, x: u16, y: u16, label: impl Into<Cow<'static, str>>, target: Target) {
        let label = label.into();
        let width = unicode_width(&label);
        let rect = Rect::new(x, y, width, 1);
        self.hits.push((rect, target.clone()));
        self.buttons.push(Button {
            rect,
            label,
            target,
        });
    }

    pub fn hit(&self, pos: Position) -> Option<&Target> {
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(pos))
            .map(|(_, t)| t)
    }

    pub fn pane_at(&self, pos: Position) -> Option<&PaneView> {
        self.panes.iter().find(|p| p.rect.contains(pos))
    }

    pub fn pane(&self, id: PaneId) -> Option<&PaneView> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Tab under `pos` while dragging a tab: by row in the sidebar, by column
    /// in the top bar, clamped to the visible tabs.
    pub fn tab_at(&self, pos: Position) -> Option<usize> {
        if self.topbar.is_none() {
            return self.tab_at_row(pos.y);
        }
        let (_, first) = self.top_tabs.first()?;
        let (_, last) = self.top_tabs.last()?;
        let x = pos.x.clamp(first.x, last.right() - 1);
        self.top_tabs
            .iter()
            .find(|(_, r)| (r.x..r.right()).contains(&x))
            .map(|(i, _)| *i)
    }

    /// Index of the tab owning the sidebar row at screen row `y`, clamped to visible rows.
    pub fn tab_at_row(&self, y: u16) -> Option<usize> {
        let (_, first) = self.rows.first()?;
        let (_, last) = self.rows.last()?;
        let y = y.clamp(first.y, last.y);
        self.rows.get((y - first.y) as usize).map(|(r, _)| r.tab())
    }
}

pub fn unicode_width(s: &str) -> u16 {
    s.chars().count() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_tooltips() {
        let id = PaneId::default();
        for t in [
            Target::NewTab,
            Target::ProfileMenu,
            Target::Help,
            Target::ToggleTabBar,
            Target::HideSidebar,
            Target::ShowSidebar,
            Target::Quit,
            Target::SidebarNarrow,
            Target::SidebarWiden,
            Target::ToggleTitles,
            Target::TabRename(0),
            Target::TabClose(0),
            Target::ClosePane(id),
            Target::SplitV(id),
            Target::SplitH(id),
            Target::Zoom(id),
            Target::Wide(id),
            Target::Sync(id),
            Target::Search(id),
            Target::SearchPrev,
            Target::SearchNext,
            Target::SearchClose,
        ] {
            assert!(t.tooltip().is_some(), "{t:?}");
        }
        assert!(Target::Tab(0).tooltip().is_none());
    }

    #[test]
    fn split_screen_widths() {
        let s = split_screen(Rect::new(0, 0, 100, 30), 20, true, TabBar::Side);
        assert_eq!(s.sidebar.unwrap().width, 20);
        assert_eq!(s.border.unwrap().x, 20);
        assert_eq!((s.main.x, s.main.width), (21, 79));
        let s = split_screen(Rect::new(0, 0, 100, 30), 20, false, TabBar::Side);
        assert!(s.sidebar.is_none());
        assert_eq!(s.main.width, 100);
    }

    #[test]
    fn split_screen_top() {
        let s = split_screen(Rect::new(0, 0, 100, 30), 20, true, TabBar::Top);
        assert!(s.sidebar.is_none() && s.border.is_none());
        assert_eq!(s.topbar, Some(Rect::new(0, 0, 100, 1)));
        assert_eq!(s.main, Rect::new(0, 1, 100, 29));
        let s = split_screen(Rect::new(0, 0, 100, 30), 20, false, TabBar::Top);
        assert!(s.topbar.is_none());
        assert_eq!(s.main.height, 30);
    }

    #[test]
    fn top_first_keeps_active_visible() {
        let w = [10, 10, 10, 10, 10];
        assert_eq!(top_first(&w, 0, 25), 0);
        assert_eq!(top_first(&w, 1, 25), 0);
        assert_eq!(top_first(&w, 4, 25), 3);
        assert_eq!(top_first(&w, 4, 100), 0);
        assert_eq!(top_first(&w, 2, 5), 2);
    }

    #[test]
    fn tab_at_in_top_bar() {
        let v = View {
            topbar: Some(Rect::new(0, 0, 80, 1)),
            top_tabs: vec![(2, Rect::new(1, 0, 8, 1)), (3, Rect::new(9, 0, 6, 1))],
            ..Default::default()
        };
        assert_eq!(v.tab_at(Position::new(0, 0)), Some(2));
        assert_eq!(v.tab_at(Position::new(9, 0)), Some(3));
        assert_eq!(v.tab_at(Position::new(70, 5)), Some(3));
    }

    #[test]
    fn pane_view_reserves_scrollbar() {
        let pv = pane_view(1, Rect::new(10, 0, 40, 20), false, true);
        assert_eq!(pv.content, Rect::new(10, 1, 39, 19));
        assert_eq!(pv.scrollbar, Some(Rect::new(49, 1, 1, 19)));
        assert_eq!(pv.hscrollbar, None);
    }

    #[test]
    fn wide_pane_view_has_hscrollbar() {
        let pv = pane_view(1, Rect::new(10, 0, 40, 20), true, true);
        assert_eq!(pv.content, Rect::new(10, 1, 39, 18));
        assert_eq!(pv.scrollbar, Some(Rect::new(49, 1, 1, 18)));
        assert_eq!(pv.hscrollbar, Some(Rect::new(10, 19, 39, 1)));
    }

    #[test]
    fn pane_view_without_title() {
        let pv = pane_view(1, Rect::new(10, 0, 40, 20), false, false);
        assert_eq!(pv.title, None);
        assert_eq!(pv.content, Rect::new(10, 0, 39, 20));
        assert_eq!(pv.close, Some(Rect::new(49, 0, 1, 1)));
        assert_eq!(pv.scrollbar, Some(Rect::new(49, 1, 1, 19)));
        assert_eq!(
            pane_view(1, Rect::new(10, 0, 40, 20), false, true).close,
            None
        );
    }

    #[test]
    fn pane_bar_rows_wrap() {
        assert_eq!(pane_bar_rows(18), 2);
        assert_eq!(pane_bar_rows(17), 2);
        assert_eq!(pane_bar_rows(9), 2);
        assert_eq!(pane_bar_rows(8), 0);
        let size = Rect::new(0, 0, 100, 30);
        assert_eq!(tab_capacity(size, 24, true, TabBar::Side, false), 27);
        assert_eq!(tab_capacity(size, 24, true, TabBar::Side, true), 25);
        assert_eq!(tab_capacity(size, 14, true, TabBar::Side, true), 25);
        assert_eq!(tab_capacity(size, 24, true, TabBar::Top, true), 0);
    }

    #[test]
    fn bar_position_mapping() {
        let bar = Rect::new(0, 10, 1, 12); // track rows 11..=20
        assert_eq!(bar_pos(bar, 11, 100), 0);
        assert_eq!(bar_pos(bar, 20, 100), 100);
        assert_eq!(bar_pos(bar, 0, 100), 0);
        assert_eq!(bar_pos(bar, 30, 100), 100);
    }

    #[test]
    fn sidebar_rows_tree() {
        use crate::layout::Dir;
        let mut t0 = Tab::new(1);
        t0.root.split(1, Dir::Vertical, 2);
        t0.root.split(2, Dir::Horizontal, 3);
        let tabs = vec![Tab::new(0), t0];
        let rows = sidebar_rows(&tabs);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Tab(0),
                SidebarRow::Tab(1),
                SidebarRow::Pane {
                    tab: 1,
                    id: 1,
                    last: false
                },
                SidebarRow::Pane {
                    tab: 1,
                    id: 2,
                    last: false
                },
                SidebarRow::Pane {
                    tab: 1,
                    id: 3,
                    last: true
                },
            ]
        );
    }

    #[test]
    fn tab_at_row_maps_pane_rows() {
        let v = View {
            rows: vec![
                (SidebarRow::Tab(0), Rect::new(0, 1, 10, 1)),
                (SidebarRow::Tab(1), Rect::new(0, 2, 10, 1)),
                (
                    SidebarRow::Pane {
                        tab: 1,
                        id: 5,
                        last: true,
                    },
                    Rect::new(0, 3, 10, 1),
                ),
            ],
            ..Default::default()
        };
        assert_eq!(v.tab_at_row(3), Some(1));
        assert_eq!(v.tab_at_row(9), Some(1));
        assert_eq!(v.tab_at_row(0), Some(0));
    }

    #[test]
    fn hit_prefers_last_pushed() {
        let mut v = View::default();
        v.hits.push((Rect::new(0, 0, 10, 1), Target::Tab(0)));
        v.button(8, 0, "×", Target::TabClose(0));
        assert_eq!(v.hit(Position::new(8, 0)), Some(&Target::TabClose(0)));
        assert_eq!(v.hit(Position::new(2, 0)), Some(&Target::Tab(0)));
        assert_eq!(v.hit(Position::new(2, 5)), None);
    }
}

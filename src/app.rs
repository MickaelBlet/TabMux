use std::collections::HashMap;
use std::io::Write;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

use crate::event::AppEvent;
use crate::input::encode_key;
use crate::layout::{Dir, PaneId, Path, Zone};
use crate::marks;
use crate::mouse;
use crate::pane::{Pane, SpawnOpts, WIDE_COLS};
use crate::search::{self, Search};
use crate::selection::{self, Selection};
use crate::tab::Tab;
use crate::view::{self, SidebarRow, TabBar, Target, View};
use crate::workspace::Workspace;

pub const MIN_SIDEBAR: u16 = 14;
const DEFAULT_SIDEBAR: u16 = 30;
const SCROLL_STEP: isize = 3;
const HSCROLL_STEP: i32 = 8;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
const STATUS_TTL: Duration = Duration::from_secs(2);

pub fn clamp_sidebar(width: u16, total: u16) -> u16 {
    let max = (total / 2).max(MIN_SIDEBAR);
    width.clamp(MIN_SIDEBAR, max)
}

pub enum Mode {
    Normal,
    Prefix,
    Rename(String),
    Help,
    /// Keyboard goes to the search bar of `App::search`.
    Search,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Drag {
    None,
    Sidebar,
    Split(Path),
    Tab(usize),
    /// Pane dragged by its title; `drop` is where it lands if released now.
    Pane {
        id: PaneId,
        drop: Option<DropTarget>,
    },
    Select,
    /// Mouse events forwarded to the app running in this pane.
    Forward(PaneId),
    Scrollbar(PaneId),
    HScrollbar(PaneId),
    TabScrollbar,
}

/// Where a pane dragged by its title can be dropped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropTarget {
    /// Next to (or swapped with) a pane, in any tab.
    Pane(PaneId, Zone),
    /// Right side of a tab's whole layout (sidebar tab row).
    Tab(usize),
    /// Into a new tab of its own.
    NewTab,
}

/// Open profile menu: anchored at the click that opened it.
#[derive(Clone, Copy, Debug)]
pub struct Menu {
    pub anchor: Position,
    /// Keyboard-selected item.
    pub sel: usize,
}

#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub struct App {
    pub panes: HashMap<PaneId, Pane>,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub sidebar_width: u16,
    pub sidebar_visible: bool,
    /// Pane title rows shown while the tab list is visible.
    pub titles_visible: bool,
    pub tab_bar: TabBar,
    pub mode: Mode,
    pub drag: Drag,
    pub hover: Option<Position>,
    pub selection: Option<Selection>,
    pub search: Option<Search>,
    /// Shell profiles from the workspace file, offered by the `[⌵]` menu.
    pub profiles: Vec<(String, SpawnOpts)>,
    pub menu: Option<Menu>,
    pub view: View,
    pub tab_scroll: usize,
    /// (active tab, focused pane) at the last scroll update.
    last_active: Option<(usize, PaneId)>,
    pub quit: bool,
    /// Draw `[x]` buttons as Nerd Font pills (`--nerd-font`).
    pub nerd_font: bool,
    pub size: Rect,
    status: Option<(String, Instant)>,
    clipboard: String,
    /// Time, position and count (1..=3) of the last click.
    last_click: Option<(Instant, Position, u8)>,
    next_id: PaneId,
    tx: Sender<AppEvent>,
}

impl App {
    pub fn new(size: Rect, tx: Sender<AppEvent>, workspace: Option<Workspace>) -> Result<Self> {
        let mut app = Self {
            panes: HashMap::new(),
            tabs: Vec::new(),
            active: 0,
            sidebar_width: clamp_sidebar(DEFAULT_SIDEBAR, size.width),
            sidebar_visible: true,
            titles_visible: true,
            tab_bar: TabBar::default(),
            mode: Mode::Normal,
            drag: Drag::None,
            hover: None,
            selection: None,
            search: None,
            profiles: Vec::new(),
            menu: None,
            view: View::default(),
            tab_scroll: 0,
            last_active: None,
            quit: false,
            nerd_font: false,
            size,
            status: None,
            clipboard: String::new(),
            last_click: None,
            next_id: 0,
            tx,
        };
        match workspace {
            Some(ws) => app.load_workspace(&ws)?,
            None => app.new_tab()?,
        }
        app.refresh();
        Ok(app)
    }

    /// Resize every PTY to its current rect and rebuild the view.
    /// Called before each redraw; `Pane::resize` is a no-op when unchanged.
    pub fn refresh(&mut self) {
        let main = self.main_area();
        let wide: std::collections::HashSet<PaneId> = self
            .panes
            .iter()
            .filter(|(_, p)| p.wide)
            .map(|(id, _)| *id)
            .collect();
        let is_wide = |id: PaneId| wide.contains(&id);
        let all_titles = self.titles_shown();
        let search = self.search.as_ref().map(|s| s.pane);
        let title = |id: PaneId| all_titles || search == Some(id);
        for tab in &self.tabs {
            let zoomed = tab.zoom.and_then(|z| {
                view::pane_views(tab, main, true, is_wide, title)
                    .0
                    .pop()
                    .map(|pv| (z, pv.content))
            });
            for pv in view::pane_views(tab, main, false, is_wide, title).0 {
                let content = match zoomed {
                    Some((z, c)) if z == pv.id => c,
                    _ => pv.content,
                };
                if let Some(p) = self.panes.get_mut(&pv.id) {
                    let cols = if p.wide { WIDE_COLS } else { content.width };
                    p.resize(content.height, cols);
                }
            }
        }
        self.update_bells();
        self.update_tab_scroll();
        self.view = View::build(self);
        self.update_hscroll();
        if self.search.as_ref().is_some_and(|s| s.dirty) {
            self.search_recompute(true);
        }
    }

    /// Flag background tabs whose panes rang the bell or produced output;
    /// the active tab is cleared.
    fn update_bells(&mut self) {
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            let (rang, out) = tab
                .root
                .leaves()
                .iter()
                .filter_map(|id| self.panes.get(id))
                .fold((false, false), |(b, o), p| {
                    (p.take_bell() | b, p.take_output() | o)
                });
            let bg = i != self.active;
            tab.bell = bg && (tab.bell || rang);
            tab.activity = bg && (tab.activity || out);
        }
    }

    /// True while some tab is blinking (the UI needs periodic redraws).
    pub fn bell_pending(&self) -> bool {
        self.tabs.iter().any(|t| t.bell)
    }

    // ---- search -------------------------------------------------------

    fn open_search(&mut self, id: PaneId) {
        self.focus(id);
        if self.search.as_ref().is_none_or(|s| s.pane != id) {
            self.search = Some(Search::new(id));
        }
        self.mode = Mode::Search;
    }

    fn close_search(&mut self) {
        self.search = None;
        if matches!(self.mode, Mode::Search) {
            self.mode = Mode::Normal;
        }
    }

    fn search_recompute(&mut self, keep: bool) {
        let Some(s) = &mut self.search else {
            return;
        };
        s.dirty = false;
        let Some(p) = self.panes.get(&s.pane) else {
            return;
        };
        let (lines, _) = p.history();
        s.set_matches(search::find(&lines, &s.query), keep);
    }

    /// Scroll the searched pane so the current match is visible.
    fn search_reveal(&mut self) {
        let Some(s) = &self.search else {
            return;
        };
        let (id, Some(m)) = (s.pane, s.current_match()) else {
            return;
        };
        let width = self.view.pane(id).map_or(1, |p| p.content.width.max(1));
        let Some(p) = self.panes.get_mut(&id) else {
            return;
        };
        let (_, max) = p.scroll_info();
        let rows = p.parser.lock().unwrap().screen().size().0 as usize;
        // Put the match about a third of the way down the pane.
        let offset = if m.line >= max {
            0
        } else {
            (max - m.line + rows / 3).min(max)
        };
        p.set_scroll(offset);
        if p.wide {
            p.follow = false;
            if m.col < p.hscroll || m.col + m.len > p.hscroll + width {
                p.hscroll = m.col.saturating_sub(width / 3);
            }
        }
    }

    fn search_step(&mut self, delta: isize) {
        if let Some(s) = &mut self.search {
            s.step(delta);
        }
        self.search_reveal();
    }

    fn search_edit(&mut self, edit: impl FnOnce(&mut String)) {
        if let Some(s) = &mut self.search {
            edit(&mut s.query);
        }
        self.search_recompute(false);
        self.search_reveal();
    }

    // ---- prompt navigation ---------------------------------------------

    /// Scroll to the previous (`-1`) or next (`+1`) prompt, like VS Code's
    /// Ctrl+Up / Ctrl+Down. Needs shell integration (OSC 133;A marks).
    fn jump_prompt(&mut self, id: PaneId, delta: isize) {
        let Some(p) = self.panes.get_mut(&id) else {
            return;
        };
        let marks = p.prompt_marks();
        if marks.is_empty() {
            self.set_status("no prompt marks (shell integration off?)".into());
            return;
        }
        let (offset, max) = p.scroll_info();
        let cursor = max + p.parser.lock().unwrap().screen().cursor_position().0 as usize;
        // Start from the last jump if the view hasn't moved since, else from the view top.
        let from = match p.jump {
            Some((line, off)) if off == offset => line,
            _ if offset == 0 => cursor,
            _ => max - offset,
        };
        match marks::step(&marks, from, delta) {
            Some(line) => {
                let off = max.saturating_sub(line);
                p.set_scroll(off);
                p.jump = Some((line, off));
            }
            None if delta > 0 => {
                p.set_scroll(0);
                p.jump = None;
            }
            None => {}
        }
    }

    /// History index of the prompt last jumped to, while the view still shows it.
    pub fn jumped_prompt(&self, id: PaneId) -> Option<usize> {
        let p = self.panes.get(&id)?;
        let (line, off) = p.jump?;
        (p.scroll_info().0 == off).then_some(line)
    }

    /// Every pane has a title row: asked for, or forced when the tab list
    /// is hidden (titles then carry the pane buttons).
    pub fn titles_shown(&self) -> bool {
        let s = view::split_screen(
            self.size,
            self.sidebar_width,
            self.sidebar_visible,
            self.tab_bar,
        );
        self.titles_visible || (s.sidebar.is_none() && s.topbar.is_none())
    }

    /// Pane `id` has a title row (the search bar needs one).
    pub fn shows_title(&self, id: PaneId) -> bool {
        self.titles_shown() || self.search.as_ref().is_some_and(|s| s.pane == id)
    }

    pub fn is_wide(&self, id: PaneId) -> bool {
        self.panes.get(&id).is_some_and(|p| p.wide)
    }

    /// Clamp horizontal scroll of visible wide panes and follow the cursor if asked.
    fn update_hscroll(&mut self) {
        for pv in &self.view.panes {
            let Some(p) = self.panes.get_mut(&pv.id) else {
                continue;
            };
            if !p.wide {
                p.hscroll = 0;
                continue;
            }
            let width = pv.content.width.max(1);
            if p.follow {
                let (_, col) = p.parser.lock().unwrap().screen().cursor_position();
                if col < p.hscroll {
                    p.hscroll = col.saturating_sub(width / 4);
                } else if col >= p.hscroll + width {
                    p.hscroll = col + 1 + width / 4 - width;
                }
            }
            p.hscroll = p.hscroll.min(WIDE_COLS.saturating_sub(width));
        }
    }

    fn toggle_wide(&mut self, id: PaneId) {
        if let Some(p) = self.panes.get_mut(&id) {
            p.wide = !p.wide;
            p.hscroll = 0;
            p.follow = true;
        }
        self.selection = None;
    }

    fn hscroll_by(&mut self, id: PaneId, delta: i32) {
        if let Some(p) = self.panes.get_mut(&id).filter(|p| p.wide) {
            p.follow = false;
            p.hscroll = (p.hscroll as i32 + delta).clamp(0, WIDE_COLS as i32) as u16;
        }
        self.selection = None;
    }

    /// Click or drag on a pane's horizontal scrollbar.
    fn on_pane_hbar(&mut self, id: PaneId, x: u16, allow_arrows: bool) {
        let Some(pv) = self.view.pane(id) else {
            return;
        };
        let (Some(bar), width) = (pv.hscrollbar, pv.content.width) else {
            return;
        };
        if allow_arrows && x == bar.x {
            self.hscroll_by(id, -HSCROLL_STEP);
        } else if allow_arrows && x == bar.right() - 1 {
            self.hscroll_by(id, HSCROLL_STEP);
        } else if let Some(p) = self.panes.get_mut(&id) {
            let max = WIDE_COLS.saturating_sub(width) as usize;
            p.follow = false;
            p.hscroll = view::bar_pos(view::transpose(bar), x, max) as u16;
            self.selection = None;
        }
    }

    /// Keep the sidebar scroll in range, and the active tab (or its focused
    /// pane row) visible when the active tab or focus changes.
    fn update_tab_scroll(&mut self) {
        let cap = view::tab_capacity(
            self.size,
            self.sidebar_width,
            self.sidebar_visible,
            self.tab_bar,
            !self.titles_visible,
        )
        .max(1);
        let rows = view::sidebar_rows(&self.tabs);
        let key = self.focused_id().map(|f| (self.active, f));
        if key.is_some() && self.last_active != key {
            self.last_active = key;
            let focus = key.map(|k| k.1);
            let target = rows
                .iter()
                .position(|r| matches!(r, SidebarRow::Pane { id, .. } if Some(*id) == focus))
                .or_else(|| rows.iter().position(|r| *r == SidebarRow::Tab(self.active)));
            if let Some(t) = target {
                // Prefer keeping the tab header visible along with the pane.
                let head = rows
                    .iter()
                    .position(|r| *r == SidebarRow::Tab(self.active))
                    .unwrap_or(t);
                if head < self.tab_scroll {
                    self.tab_scroll = head;
                }
                if t >= self.tab_scroll + cap {
                    self.tab_scroll = t + 1 - cap;
                }
            }
        }
        self.tab_scroll = self.tab_scroll.min((rows.len() + 1).saturating_sub(cap));
    }

    fn tab_scroll_max(&self) -> usize {
        let cap = view::tab_capacity(
            self.size,
            self.sidebar_width,
            self.sidebar_visible,
            self.tab_bar,
            !self.titles_visible,
        );
        (view::sidebar_rows(&self.tabs).len() + 1).saturating_sub(cap)
    }

    /// Click or drag on a pane scrollbar. Arrow cells step, the track jumps.
    fn on_pane_bar(&mut self, id: PaneId, y: u16, allow_arrows: bool) {
        let Some(bar) = self.view.pane(id).and_then(|p| p.scrollbar) else {
            return;
        };
        let Some(p) = self.panes.get_mut(&id) else {
            return;
        };
        self.selection = None;
        if allow_arrows && y == bar.y {
            p.scroll(SCROLL_STEP);
        } else if allow_arrows && y == bar.bottom() - 1 {
            p.scroll(-SCROLL_STEP);
        } else {
            let (_, max) = p.scroll_info();
            // Top of the track = oldest line = maximum offset.
            p.set_scroll(max - view::bar_pos(bar, y, max));
        }
    }

    fn on_tab_bar(&mut self, y: u16, allow_arrows: bool) {
        let Some(bar) = self.view.tab_scrollbar else {
            return;
        };
        let max = self.tab_scroll_max();
        self.tab_scroll = if allow_arrows && y == bar.y {
            self.tab_scroll.saturating_sub(1)
        } else if allow_arrows && y == bar.bottom() - 1 {
            (self.tab_scroll + 1).min(max)
        } else {
            view::bar_pos(bar, y, max)
        };
    }

    fn main_area(&self) -> Rect {
        view::split_screen(
            self.size,
            self.sidebar_width,
            self.sidebar_visible,
            self.tab_bar,
        )
        .main
    }

    pub fn status(&self) -> Option<&str> {
        self.status
            .as_ref()
            .filter(|(_, t)| t.elapsed() < STATUS_TTL)
            .map(|(s, _)| s.as_str())
    }

    fn set_status(&mut self, s: String) {
        self.status = Some((s, Instant::now()));
    }

    // ---- tabs & panes -------------------------------------------------

    fn spawn_pane(&mut self, opts: &SpawnOpts) -> Result<PaneId> {
        let id = self.next_id;
        self.next_id += 1;
        let main = self.main_area();
        let pane = Pane::spawn(
            id,
            main.height.saturating_sub(1),
            main.width,
            self.tx.clone(),
            opts,
        )?;
        self.panes.insert(id, pane);
        Ok(id)
    }

    fn new_tab(&mut self) -> Result<()> {
        self.new_tab_with(&SpawnOpts::default())
    }

    fn new_tab_with(&mut self, opts: &SpawnOpts) -> Result<()> {
        let id = self.spawn_pane(opts)?;
        self.tabs.push(Tab::new(id));
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    fn load_workspace(&mut self, ws: &Workspace) -> Result<()> {
        self.profiles = ws
            .profile
            .iter()
            .map(|p| {
                let opts = SpawnOpts {
                    cwd: ws.resolve_cwd(None, p.cwd.as_deref()),
                    command: p.command.clone(),
                    env: p.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    shell: p.shell.clone(),
                };
                (p.name.clone(), opts)
            })
            .collect();
        for spec in &ws.tab {
            let mut spawn = |p: &crate::workspace::PaneSpec| {
                let env = spec
                    .env
                    .iter()
                    .chain(&p.env)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                let opts = SpawnOpts {
                    cwd: ws.resolve_cwd(spec.cwd.as_deref(), p.cwd.as_deref()),
                    command: p.command.clone(),
                    env,
                    shell: None,
                };
                self.spawn_pane(&opts)
            };
            let default = crate::workspace::NodeSpec::Pane(Default::default());
            let (root, focus) = spec.layout.as_ref().unwrap_or(&default).build(&mut spawn)?;
            let focus = focus.unwrap_or_else(|| root.leaves()[0]);
            self.tabs.push(Tab {
                name: spec.name.clone(),
                root,
                focus,
                zoom: None,
                sync: false,
                bell: false,
                activity: false,
            });
        }
        self.active = ws.active.unwrap_or(0).min(self.tabs.len() - 1);
        self.titles_visible = ws.titles.unwrap_or(true);
        Ok(())
    }

    /// Open a new tab with profile `i` and close the menu.
    fn spawn_profile(&mut self, i: usize) -> Result<()> {
        self.menu = None;
        let Some((name, opts)) = self.profiles.get(i).cloned() else {
            return Ok(());
        };
        if let Err(e) = self.new_tab_with(&opts) {
            self.set_status(format!("profile {name}: {e:#}"));
        }
        Ok(())
    }

    fn on_menu_key(&mut self, key: KeyEvent) -> Result<()> {
        let n = self.profiles.len().max(1);
        let Some(menu) = &mut self.menu else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => self.menu = None,
            KeyCode::Up => menu.sel = (menu.sel + n - 1) % n,
            KeyCode::Down => menu.sel = (menu.sel + 1) % n,
            KeyCode::Enter => {
                let i = menu.sel;
                self.spawn_profile(i)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(idx);
        for id in tab.root.leaves() {
            self.panes.remove(&id);
        }
        self.after_tab_removed(idx);
    }

    fn after_tab_removed(&mut self, idx: usize) {
        if self.tabs.is_empty() {
            self.quit = true;
            return;
        }
        if self.active > idx || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
    }

    fn split(&mut self, target: PaneId, dir: Dir) -> Result<()> {
        let Some(ti) = self.tab_of(target) else {
            return Ok(());
        };
        let new_id = self.spawn_pane(&SpawnOpts::default())?;
        let tab = &mut self.tabs[ti];
        tab.root.split(target, dir, new_id);
        tab.focus = new_id;
        tab.zoom = None;
        self.active = ti;
        Ok(())
    }

    /// Drop target under `pos` while dragging pane `id`.
    fn drop_target(&self, id: PaneId, pos: Position) -> Option<DropTarget> {
        if let Some(pv) = self.view.pane_at(pos) {
            return (pv.id != id).then(|| DropTarget::Pane(pv.id, Zone::at(pv.rect, pos.x, pos.y)));
        }
        if let Some(&(row, _)) = self.view.rows.iter().find(|(_, r)| r.contains(pos)) {
            return match row {
                SidebarRow::Tab(i) => self.drop_on_tab(id, i),
                SidebarRow::Pane { id: p, .. } => {
                    (p != id).then(|| DropTarget::Pane(p, self.row_zone(id, p)))
                }
            };
        }
        if let Some(&(i, _)) = self.view.top_tabs.iter().find(|(_, r)| r.contains(pos)) {
            return self.drop_on_tab(id, i);
        }
        let alone = self
            .tab_of(id)
            .is_some_and(|ti| self.tabs[ti].root.leaves().len() == 1);
        (self.view.hit(pos) == Some(&Target::NewTab) && !alone).then_some(DropTarget::NewTab)
    }

    /// Side pane `id` occupies next to its sibling, kept when dropped from the
    /// sidebar. Defaults to the right.
    fn side_of(&self, id: PaneId) -> Zone {
        self.tab_of(id)
            .and_then(|ti| self.tabs[ti].root.side_of(id))
            .unwrap_or(Zone::Right)
    }

    /// Zone for dropping pane `id` on the sidebar row of pane `target`: keeps
    /// the split direction of `id`; within a tab, lands before `target` when
    /// moved up the list and after it when moved down, so two panes swap.
    fn row_zone(&self, id: PaneId, target: PaneId) -> Zone {
        let side = self.side_of(id);
        let Some(ti) = self
            .tab_of(id)
            .filter(|&ti| self.tab_of(target) == Some(ti))
        else {
            return side;
        };
        let leaves = self.tabs[ti].root.leaves();
        let pos = |p| leaves.iter().position(|&l| l == p);
        let up = pos(target) < pos(id);
        match (side, up) {
            (Zone::Top | Zone::Bottom, true) => Zone::Top,
            (Zone::Top | Zone::Bottom, false) => Zone::Bottom,
            (_, true) => Zone::Left,
            (_, false) => Zone::Right,
        }
    }

    /// Dropping pane `id` on tab `i`, unless it is that tab's only pane.
    fn drop_on_tab(&self, id: PaneId, i: usize) -> Option<DropTarget> {
        let alone = self.tabs.get(i).is_some_and(|t| t.root.leaves() == [id]);
        (!alone).then_some(DropTarget::Tab(i))
    }

    fn drop_pane(&mut self, id: PaneId, drop: DropTarget) {
        use crate::layout::Node;
        let Some(src) = self.tab_of(id) else { return };
        let side = self.side_of(id);
        // Destination identified by a pane, stable when the source tab disappears.
        enum Dest {
            Pane(PaneId, Zone),
            Root(PaneId),
            New,
        }
        let dest = match drop {
            DropTarget::Pane(t, _) if t == id => return,
            DropTarget::Pane(t, z) => Dest::Pane(t, z),
            DropTarget::Tab(i) => {
                let Some(anchor) = self
                    .tabs
                    .get(i)
                    .and_then(|t| t.root.leaves().into_iter().find(|&l| l != id))
                else {
                    return;
                };
                Dest::Root(anchor)
            }
            DropTarget::NewTab => Dest::New,
        };

        match dest {
            Dest::Pane(t, z) if self.tab_of(t) == Some(src) => {
                let tab = &mut self.tabs[src];
                let root = std::mem::replace(&mut tab.root, Node::Leaf(0));
                tab.root = root.move_pane(id, t, z);
            }
            Dest::Pane(t, Zone::Center) => {
                // Swap across tabs.
                let Some(dt) = self.tab_of(t) else { return };
                self.tabs[src].root.swap(id, t);
                self.tabs[dt].root.swap(id, t);
                let tab = &mut self.tabs[src];
                if tab.focus == id {
                    tab.focus = t;
                }
                if tab.zoom == Some(id) {
                    tab.zoom = Some(t);
                }
            }
            _ => {
                if self.tabs[src].root.leaves() == [id] && matches!(dest, Dest::New) {
                    return;
                }
                let tab = &mut self.tabs[src];
                let root = std::mem::replace(&mut tab.root, Node::Leaf(0));
                match root.remove(id) {
                    Some(root) => {
                        tab.root = root;
                        if tab.zoom == Some(id) {
                            tab.zoom = None;
                        }
                        if tab.focus == id {
                            tab.focus = tab.root.leaves()[0];
                        }
                    }
                    None => {
                        self.tabs.remove(src);
                        self.after_tab_removed(src);
                    }
                }
                match dest {
                    Dest::Pane(t, z) => {
                        let Some(dt) = self.tab_of(t) else { return };
                        self.tabs[dt].root.insert(t, z, id);
                    }
                    Dest::Root(anchor) => {
                        let Some(dt) = self.tab_of(anchor) else {
                            return;
                        };
                        let tab = &mut self.tabs[dt];
                        let root = Box::new(std::mem::replace(&mut tab.root, Node::Leaf(0)));
                        let leaf = Box::new(Node::Leaf(id));
                        let (dir, a, b) = match side {
                            Zone::Left => (Dir::Vertical, leaf, root),
                            Zone::Top => (Dir::Horizontal, leaf, root),
                            Zone::Bottom => (Dir::Horizontal, root, leaf),
                            _ => (Dir::Vertical, root, leaf),
                        };
                        tab.root = Node::Split {
                            dir,
                            ratio: 0.5,
                            a,
                            b,
                        };
                    }
                    Dest::New => self.tabs.push(Tab::new(id)),
                }
            }
        }
        if let Some(ti) = self.tab_of(id) {
            self.active = ti;
            let tab = &mut self.tabs[ti];
            tab.focus = id;
            tab.zoom = None;
        }
    }

    fn close_pane(&mut self, id: PaneId) {
        let Some(ti) = self.tab_of(id) else { return };
        self.panes.remove(&id);
        if self.selection.as_ref().is_some_and(|s| s.pane == id) {
            self.selection = None;
        }
        if self.search.as_ref().is_some_and(|s| s.pane == id) {
            self.close_search();
        }
        let tab = &mut self.tabs[ti];
        let root = std::mem::replace(&mut tab.root, crate::layout::Node::Leaf(0));
        match root.remove(id) {
            Some(root) => {
                tab.root = root;
                if tab.zoom == Some(id) {
                    tab.zoom = None;
                }
                if tab.focus == id {
                    tab.focus = tab.root.leaves()[0];
                }
            }
            None => {
                self.tabs.remove(ti);
                self.after_tab_removed(ti);
            }
        }
    }

    fn tab_of(&self, id: PaneId) -> Option<usize> {
        self.tabs.iter().position(|t| t.root.contains(id))
    }

    fn focused_id(&self) -> Option<PaneId> {
        self.tabs.get(self.active).map(|t| t.focus)
    }

    /// Panes receiving keyboard input: the focused one, or all panes of the
    /// active tab in multicursor mode.
    fn input_ids(&self) -> Vec<PaneId> {
        match self.tabs.get(self.active) {
            Some(t) if t.sync => t.root.leaves(),
            Some(t) => vec![t.focus],
            None => Vec::new(),
        }
    }

    fn toggle_sync(&mut self, id: PaneId) {
        if let Some(ti) = self.tab_of(id) {
            let t = &mut self.tabs[ti];
            t.sync = !t.sync;
            let on = t.sync;
            self.set_status(format!("multicursor {}", if on { "on" } else { "off" }));
        }
    }

    fn focused(&mut self) -> Option<&mut Pane> {
        let id = self.focused_id()?;
        self.panes.get_mut(&id)
    }

    fn focus(&mut self, id: PaneId) {
        if let Some(ti) = self.tab_of(id) {
            self.active = ti;
            let tab = &mut self.tabs[ti];
            tab.focus = id;
            if tab.zoom.is_some_and(|z| z != id) {
                tab.zoom = None;
            }
        }
    }

    fn toggle_zoom(&mut self, id: PaneId) {
        if let Some(ti) = self.tab_of(id) {
            let tab = &mut self.tabs[ti];
            tab.zoom = if tab.zoom == Some(id) { None } else { Some(id) };
            tab.focus = id;
        }
    }

    fn cycle_tab(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        if n > 0 {
            self.active = (self.active as isize + delta).rem_euclid(n) as usize;
        }
    }

    /// Scroll the focused pane's history by one page (keeping one line of overlap).
    fn scroll_page(&mut self, up: bool) {
        let Some(id) = self.focused_id() else {
            return;
        };
        let page = self
            .view
            .pane(id)
            .map_or(1, |p| p.content.height.saturating_sub(1).max(1)) as isize;
        if let Some(p) = self.panes.get_mut(&id) {
            p.scroll(if up { page } else { -page });
            self.selection = None;
        }
    }

    /// Step through every pane of every tab, in sidebar order, wrapping around.
    fn cycle_sidebar(&mut self, delta: isize) {
        let all: Vec<PaneId> = self.tabs.iter().flat_map(|t| t.root.leaves()).collect();
        let Some(cur) = self.focused_id() else {
            return;
        };
        let n = all.len() as isize;
        if let Some(i) = all.iter().position(|&id| id == cur) {
            self.focus(all[(i as isize + delta).rem_euclid(n) as usize]);
        }
    }

    fn cycle_pane(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let leaves = tab.root.leaves();
        let i = leaves.iter().position(|&l| l == tab.focus).unwrap_or(0);
        tab.focus = leaves[(i + 1) % leaves.len()];
        tab.zoom = None;
    }

    fn focus_direction(&mut self, dir: Direction) {
        let Some(cur_id) = self.focused_id() else {
            return;
        };
        let Some(cur) = self.view.pane(cur_id).map(|p| p.content) else {
            return;
        };
        let (cx, cy) = (
            cur.x as i32 + cur.width as i32 / 2,
            cur.y as i32 + cur.height as i32 / 2,
        );
        let best = self
            .view
            .panes
            .iter()
            .filter(|p| p.id != cur_id)
            .filter_map(|p| {
                let r = p.content;
                let (px, py) = (
                    r.x as i32 + r.width as i32 / 2,
                    r.y as i32 + r.height as i32 / 2,
                );
                let ok = match dir {
                    Direction::Left => r.right() <= cur.x,
                    Direction::Right => r.x >= cur.right(),
                    Direction::Up => r.bottom() <= cur.y,
                    Direction::Down => r.y >= cur.bottom(),
                };
                ok.then(|| {
                    let (main, cross) = match dir {
                        Direction::Left | Direction::Right => ((px - cx).abs(), (py - cy).abs()),
                        Direction::Up | Direction::Down => ((py - cy).abs(), (px - cx).abs()),
                    };
                    (main + 2 * cross, p.id)
                })
            })
            .min();
        if let Some((_, id)) = best {
            self.focus(id);
        }
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = to;
    }

    fn toggle_tab_bar(&mut self) {
        self.tab_bar = self.tab_bar.toggle();
        self.sidebar_visible = true;
    }

    fn set_sidebar_width(&mut self, width: u16) {
        self.sidebar_width = clamp_sidebar(width, self.size.width);
    }

    /// Display name of a tab: custom name, else the focused pane's title.
    pub fn tab_title(&self, tab: &Tab) -> String {
        tab.name.clone().unwrap_or_else(|| {
            self.panes
                .get(&tab.focus)
                .map(|p| p.title())
                .unwrap_or_default()
        })
    }

    fn start_rename(&mut self, idx: usize) {
        if let Some(t) = self.tabs.get(idx) {
            self.active = idx;
            let name = self.tab_title(t);
            self.mode = Mode::Rename(name);
        }
    }

    fn commit_rename(&mut self, buf: &str) {
        let name = buf.trim();
        if let Some(t) = self.tabs.get_mut(self.active) {
            // An empty name goes back to following the pane title.
            t.name = (!name.is_empty()).then(|| name.to_string());
        }
    }

    fn paste(&mut self, s: &str) {
        for id in self.input_ids() {
            let Some(p) = self.panes.get_mut(&id) else {
                continue;
            };
            p.reset_scroll();
            p.jump = None;
            let bracketed = p.parser.lock().unwrap().screen().bracketed_paste();
            if bracketed {
                p.write(b"\x1b[200~");
                p.write(s.as_bytes());
                p.write(b"\x1b[201~");
            } else {
                p.write(s.replace('\n', "\r").as_bytes());
            }
        }
    }

    fn copy_selection(&mut self) {
        let Some(sel) = self.selection.clone() else {
            return;
        };
        let Some(p) = self.panes.get(&sel.pane) else {
            return;
        };
        let text = sel.text(p.parser.lock().unwrap().screen());
        if text.is_empty() {
            return;
        }
        let mut out = std::io::stdout();
        let _ = out.write_all(selection::osc52(&text).as_bytes());
        let _ = out.flush();
        self.set_status(format!("copied {} chars", text.chars().count()));
        self.clipboard = text;
    }

    // ---- events -------------------------------------------------------

    pub fn handle(&mut self, ev: AppEvent) -> Result<()> {
        match ev {
            AppEvent::PtyOutput => {
                if let Some(s) = &mut self.search {
                    s.dirty = true;
                }
            }
            AppEvent::PtyExit(id) => self.close_pane(id),
            AppEvent::Input(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                self.on_key(key)?
            }
            AppEvent::Input(Event::Mouse(m)) => self.on_mouse(m)?,
            AppEvent::Input(Event::Paste(s)) => match &mut self.mode {
                Mode::Rename(buf) => buf.push_str(&s.replace(['\n', '\r'], " ")),
                _ => self.paste(&s),
            },
            AppEvent::Input(Event::Resize(w, h)) => {
                self.size = Rect::new(0, 0, w, h);
                self.menu = None;
                self.sidebar_width = clamp_sidebar(self.sidebar_width, w);
            }
            AppEvent::Input(_) => {}
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.menu.is_some() {
            return self.on_menu_key(key);
        }
        let is_prefix =
            key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL);
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => {
                let ctrl_arrow = key.modifiers == KeyModifiers::CONTROL
                    && matches!(key.code, KeyCode::Up | KeyCode::Down);
                let ctrl_page = key.modifiers == KeyModifiers::CONTROL
                    && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown);
                let shift_page = key.modifiers == KeyModifiers::SHIFT
                    && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown);
                let alt_screen = self
                    .focused()
                    .is_some_and(|p| p.parser.lock().unwrap().screen().alternate_screen());
                if is_prefix {
                    self.mode = Mode::Prefix;
                } else if ctrl_page {
                    self.cycle_sidebar(if key.code == KeyCode::PageUp { -1 } else { 1 });
                } else if shift_page && !alt_screen {
                    self.scroll_page(key.code == KeyCode::PageUp);
                } else if ctrl_arrow && !alt_screen {
                    if let Some(id) = self.focused_id() {
                        self.jump_prompt(id, if key.code == KeyCode::Up { -1 } else { 1 });
                    }
                } else {
                    for id in self.input_ids() {
                        let Some(p) = self.panes.get_mut(&id) else {
                            continue;
                        };
                        p.reset_scroll();
                        p.jump = None;
                        let app_cursor = p.parser.lock().unwrap().screen().application_cursor();
                        p.write(&encode_key(key, app_cursor));
                        p.follow = true;
                        self.selection = None;
                    }
                }
            }
            Mode::Prefix => self.on_prefix_key(key, is_prefix)?,
            Mode::Rename(mut buf) => match key.code {
                KeyCode::Enter => self.commit_rename(&buf),
                KeyCode::Esc => {}
                KeyCode::Backspace => {
                    buf.pop();
                    self.mode = Mode::Rename(buf);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.mode = Mode::Rename(String::new());
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buf.push(c);
                    self.mode = Mode::Rename(buf);
                }
                _ => self.mode = Mode::Rename(buf),
            },
            Mode::Help => {}
            Mode::Search => {
                self.mode = Mode::Search;
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => self.close_search(),
                    KeyCode::Enter | KeyCode::Up => self.search_step(-1),
                    KeyCode::Down => self.search_step(1),
                    KeyCode::Backspace => self.search_edit(|q| {
                        q.pop();
                    }),
                    KeyCode::Char('u') if ctrl => self.search_edit(String::clear),
                    KeyCode::Char(c) if !ctrl => self.search_edit(|q| q.push(c)),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn on_prefix_key(&mut self, key: KeyEvent, is_prefix: bool) -> Result<()> {
        if is_prefix {
            for id in self.input_ids() {
                if let Some(p) = self.panes.get_mut(&id) {
                    p.write(&[0x02]);
                }
            }
            return Ok(());
        }
        let focused = self.focused_id();
        match key.code {
            KeyCode::Char('c') => self.new_tab()?,
            KeyCode::Char('&') => self.close_tab(self.active),
            KeyCode::Char('x') => {
                if let Some(id) = focused {
                    self.close_pane(id);
                }
            }
            KeyCode::Char('%') | KeyCode::Char('|') => {
                if let Some(id) = focused {
                    self.split(id, Dir::Vertical)?;
                }
            }
            KeyCode::Char('"') | KeyCode::Char('-') => {
                if let Some(id) = focused {
                    self.split(id, Dir::Horizontal)?;
                }
            }
            KeyCode::Char('w') => {
                if let Some(id) = focused {
                    self.toggle_wide(id);
                }
            }
            KeyCode::Char('z') => {
                if let Some(id) = focused {
                    self.toggle_zoom(id);
                }
            }
            KeyCode::Char('m') => {
                if let Some(id) = focused {
                    self.toggle_sync(id);
                }
            }
            KeyCode::Char('o') => self.cycle_pane(),
            KeyCode::Left | KeyCode::Char('h') => self.focus_direction(Direction::Left),
            KeyCode::Right | KeyCode::Char('l') => self.focus_direction(Direction::Right),
            KeyCode::Up | KeyCode::Char('k') => self.focus_direction(Direction::Up),
            KeyCode::Down | KeyCode::Char('j') => self.focus_direction(Direction::Down),
            KeyCode::Char('n') => self.cycle_tab(1),
            KeyCode::Char('p') => self.cycle_tab(-1),
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < self.tabs.len() {
                    self.active = i;
                }
            }
            KeyCode::Char('<') => self.set_sidebar_width(self.sidebar_width.saturating_sub(2)),
            KeyCode::Char('>') => self.set_sidebar_width(self.sidebar_width + 2),
            KeyCode::Char('s') => self.sidebar_visible = !self.sidebar_visible,
            KeyCode::Char('t') => self.toggle_tab_bar(),
            KeyCode::Char('T') => self.titles_visible = !self.titles_visible,
            KeyCode::Char('r') => self.start_rename(self.active),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('/') => {
                if let Some(id) = focused {
                    self.open_search(id);
                }
            }
            KeyCode::Char('[') | KeyCode::Char(']') => {
                if let Some(id) = focused {
                    let delta = if key.code == KeyCode::Char('[') {
                        -1
                    } else {
                        1
                    };
                    self.jump_prompt(id, delta);
                }
            }
            KeyCode::Char('q') => self.quit = true,
            _ => {}
        }
        Ok(())
    }

    fn on_mouse(&mut self, m: MouseEvent) -> Result<()> {
        let pos = Position::new(m.column, m.row);
        self.hover = Some(pos);

        // Ongoing drags take precedence over hit-testing.
        match (&self.drag, m.kind) {
            (Drag::None, _) => {}
            (Drag::Forward(id), kind) => {
                let id = *id;
                self.forward_mouse(id, m);
                if matches!(kind, MouseEventKind::Up(_)) {
                    self.drag = Drag::None;
                }
                return Ok(());
            }
            (_, MouseEventKind::Drag(MouseButton::Left)) => {
                self.on_drag(pos);
                return Ok(());
            }
            (_, MouseEventKind::Up(_)) => {
                if let Drag::Pane {
                    id,
                    drop: Some(drop),
                } = self.drag
                {
                    self.drop_pane(id, drop);
                }
                if self.drag == Drag::Select {
                    if self.selection.as_ref().is_some_and(|s| !s.is_empty()) {
                        self.copy_selection();
                    } else {
                        self.selection = None;
                    }
                }
                self.drag = Drag::None;
                return Ok(());
            }
            _ => {}
        }

        let target = self.view.hit(pos).cloned();

        if let Some(menu) = &mut self.menu {
            match (m.kind, target) {
                (MouseEventKind::Moved, Some(Target::Profile(i))) => menu.sel = i,
                (MouseEventKind::Down(MouseButton::Left), Some(Target::Profile(i))) => {
                    self.spawn_profile(i)?
                }
                // Any other click (including the button again) closes the menu.
                (MouseEventKind::Down(_), _) => self.menu = None,
                _ => {}
            }
            return Ok(());
        }

        if matches!(self.mode, Mode::Help) {
            if matches!(m.kind, MouseEventKind::Down(_)) {
                self.mode = Mode::Normal;
            }
            return Ok(());
        }

        match m.kind {
            MouseEventKind::Down(button) => {
                if !matches!(self.mode, Mode::Search)
                    && let Mode::Rename(buf) = std::mem::replace(&mut self.mode, Mode::Normal)
                {
                    self.commit_rename(&buf);
                }
                // Successive clicks at the same spot count 1, 2, 3, then start over.
                let clicks = match self.last_click {
                    Some((t, p, n)) if p == pos && t.elapsed() < DOUBLE_CLICK => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), pos, clicks));
                if let Some(t) = target {
                    self.on_click(t, button, m, clicks)?;
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = m.kind == MouseEventKind::ScrollUp;
                let shift = m.modifiers.contains(KeyModifiers::SHIFT);
                let hdelta = if up { -HSCROLL_STEP } else { HSCROLL_STEP };
                match target {
                    Some(Target::HScrollbar(id)) => self.hscroll_by(id, hdelta),
                    Some(Target::PaneContent(id)) if shift && self.is_wide(id) => {
                        self.hscroll_by(id, hdelta)
                    }
                    Some(Target::PaneContent(id)) => self.on_wheel(id, m, up),
                    Some(Target::Scrollbar(id)) => {
                        if let Some(p) = self.panes.get_mut(&id) {
                            p.scroll(if up { SCROLL_STEP } else { -SCROLL_STEP });
                        }
                    }
                    // Anywhere in the sidebar: scroll the list, never switch tabs.
                    _ if self.view.sidebar.is_some_and(|r| r.contains(pos)) => {
                        self.tab_scroll = if up {
                            self.tab_scroll.saturating_sub(1)
                        } else {
                            (self.tab_scroll + 1).min(self.tab_scroll_max())
                        };
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
                let delta = if m.kind == MouseEventKind::ScrollLeft {
                    -HSCROLL_STEP
                } else {
                    HSCROLL_STEP
                };
                if let Some(Target::PaneContent(id) | Target::HScrollbar(id)) = target {
                    self.hscroll_by(id, delta);
                }
            }
            MouseEventKind::Moved => {
                if let Some(Target::PaneContent(id)) = target
                    && !m.modifiers.contains(KeyModifiers::SHIFT)
                {
                    self.forward_mouse(id, m);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn on_click(
        &mut self,
        t: Target,
        button: MouseButton,
        m: MouseEvent,
        clicks: u8,
    ) -> Result<()> {
        let left = button == MouseButton::Left;
        let double = clicks == 2;
        match t {
            Target::PaneContent(id) => {
                if matches!(self.mode, Mode::Search) {
                    // Clicking the terminal gives the keyboard back to it.
                    self.mode = Mode::Normal;
                }
                self.focus(id);
                let shift = m.modifiers.contains(KeyModifiers::SHIFT);
                if !shift && self.wants_mouse(id) {
                    self.selection = None;
                    self.forward_mouse(id, m);
                    self.drag = Drag::Forward(id);
                } else if left && clicks >= 2 {
                    // Double-click copies the word, triple-click the whole line.
                    let pos = self.content_pos(id, Position::new(m.column, m.row));
                    self.selection = pos.and_then(|(row, col)| {
                        let pane = self.panes.get(&id)?;
                        let parser = pane.parser.lock().unwrap();
                        if double {
                            Selection::word_at(id, parser.screen(), row, col)
                        } else {
                            Selection::line_at(id, parser.screen(), row)
                        }
                    });
                    self.copy_selection();
                } else if left {
                    let p = self.content_pos(id, Position::new(m.column, m.row));
                    self.selection = p.map(|p| Selection::new(id, p));
                    self.drag = Drag::Select;
                } else if button == MouseButton::Middle {
                    let clip = self.clipboard.clone();
                    self.paste(&clip);
                }
            }
            _ if !left => {}
            Target::NewTab => self.new_tab()?,
            Target::ProfileMenu => {
                self.menu = Some(Menu {
                    anchor: Position::new(m.column, m.row),
                    sel: 0,
                });
            }
            Target::Profile(i) => self.spawn_profile(i)?,
            Target::CloseMenu => self.menu = None,
            Target::Help => self.mode = Mode::Help,
            Target::CloseHelp => self.mode = Mode::Normal,
            Target::HideSidebar => self.sidebar_visible = false,
            Target::ShowSidebar => self.sidebar_visible = true,
            Target::ToggleTitles => self.titles_visible = !self.titles_visible,
            Target::Quit => self.quit = true,
            Target::SidebarNarrow => self.set_sidebar_width(self.sidebar_width.saturating_sub(2)),
            Target::SidebarWiden => self.set_sidebar_width(self.sidebar_width + 2),
            Target::SidebarBorder => self.drag = Drag::Sidebar,
            Target::ToggleTabBar => self.toggle_tab_bar(),
            Target::Tab(i) => {
                if double {
                    self.start_rename(i);
                } else {
                    self.active = i;
                    self.drag = Drag::Tab(i);
                }
            }
            Target::TabRename(i) => self.start_rename(i),
            Target::TabClose(i) => self.close_tab(i),
            Target::PaneRow(id) | Target::PaneTitle(id) => {
                if double {
                    self.toggle_zoom(id);
                } else {
                    self.focus(id);
                    self.drag = Drag::Pane { id, drop: None };
                }
            }
            Target::SplitV(id) => self.split(id, Dir::Vertical)?,
            Target::SplitH(id) => self.split(id, Dir::Horizontal)?,
            Target::Zoom(id) => self.toggle_zoom(id),
            Target::ClosePane(id) => self.close_pane(id),
            Target::Separator(path) => self.drag = Drag::Split(path),
            Target::Wide(id) => self.toggle_wide(id),
            Target::Sync(id) => self.toggle_sync(id),
            Target::Search(id) => self.open_search(id),
            Target::SearchBar => self.mode = Mode::Search,
            Target::SearchPrev => self.search_step(-1),
            Target::SearchNext => self.search_step(1),
            Target::SearchClose => self.close_search(),
            Target::HScrollbar(id) => {
                self.on_pane_hbar(id, m.column, true);
                self.drag = Drag::HScrollbar(id);
            }
            Target::Scrollbar(id) => {
                self.on_pane_bar(id, m.row, true);
                self.drag = Drag::Scrollbar(id);
            }
            Target::TabScrollbar => {
                self.on_tab_bar(m.row, true);
                self.drag = Drag::TabScrollbar;
            }
        }
        Ok(())
    }

    fn on_drag(&mut self, pos: Position) {
        match self.drag.clone() {
            Drag::Sidebar => self.set_sidebar_width(pos.x.saturating_sub(self.size.x)),
            Drag::Split(path) => {
                let Some(sep) = self.view.seps.iter().find(|s| s.path == path) else {
                    return;
                };
                let p = sep.parent;
                let ratio = match sep.dir {
                    Dir::Vertical => (pos.x.saturating_sub(p.x)) as f32 / p.width.max(1) as f32,
                    Dir::Horizontal => (pos.y.saturating_sub(p.y)) as f32 / p.height.max(1) as f32,
                };
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.root.set_ratio(&path, ratio);
                }
            }
            Drag::Tab(from) => {
                if let Some(to) = self.view.tab_at(pos) {
                    let to = to.min(self.tabs.len().saturating_sub(1));
                    if to != from {
                        self.move_tab(from, to);
                        self.drag = Drag::Tab(to);
                    }
                }
            }
            Drag::Pane { id, .. } => {
                let drop = self.drop_target(id, pos);
                self.drag = Drag::Pane { id, drop };
            }
            Drag::Select => {
                let Some(id) = self.selection.as_ref().map(|s| s.pane) else {
                    return;
                };
                if let Some(p) = self.content_pos(id, pos)
                    && let Some(s) = &mut self.selection
                {
                    s.cursor = p;
                }
            }
            Drag::Scrollbar(id) => self.on_pane_bar(id, pos.y, false),
            Drag::HScrollbar(id) => self.on_pane_hbar(id, pos.x, false),
            Drag::TabScrollbar => self.on_tab_bar(pos.y, false),
            Drag::None | Drag::Forward(_) => {}
        }
    }

    fn on_wheel(&mut self, id: PaneId, m: MouseEvent, up: bool) {
        self.selection = None;
        if !m.modifiers.contains(KeyModifiers::SHIFT) && self.wants_mouse(id) {
            self.forward_mouse(id, m);
            return;
        }
        let Some(p) = self.panes.get_mut(&id) else {
            return;
        };
        let (alt, app_cursor) = {
            let parser = p.parser.lock().unwrap();
            (
                parser.screen().alternate_screen(),
                parser.screen().application_cursor(),
            )
        };
        if alt {
            // Full-screen app without mouse support: emulate arrow keys.
            let code = if up { KeyCode::Up } else { KeyCode::Down };
            let bytes = encode_key(KeyEvent::new(code, KeyModifiers::NONE), app_cursor);
            for _ in 0..SCROLL_STEP {
                p.write(&bytes);
            }
        } else {
            p.scroll(if up { SCROLL_STEP } else { -SCROLL_STEP });
        }
    }

    fn wants_mouse(&self, id: PaneId) -> bool {
        self.panes.get(&id).is_some_and(|p| {
            p.parser.lock().unwrap().screen().mouse_protocol_mode()
                != vt100::MouseProtocolMode::None
        })
    }

    /// Position relative to a pane's content, clamped inside it.
    fn content_pos(&self, id: PaneId, pos: Position) -> Option<(u16, u16)> {
        let r = self.view.pane(id)?.content;
        if r.width == 0 || r.height == 0 {
            return None;
        }
        let hscroll = self.panes.get(&id).map_or(0, |p| p.hscroll);
        let col = pos.x.clamp(r.x, r.right() - 1) - r.x + hscroll;
        let row = pos.y.clamp(r.y, r.bottom() - 1) - r.y;
        Some((row, col))
    }

    fn forward_mouse(&mut self, id: PaneId, m: MouseEvent) {
        let Some((row, col)) = self.content_pos(id, Position::new(m.column, m.row)) else {
            return;
        };
        let Some(p) = self.panes.get_mut(&id) else {
            return;
        };
        let bytes = {
            let parser = p.parser.lock().unwrap();
            let s = parser.screen();
            mouse::encode(
                m.kind,
                m.modifiers,
                col,
                row,
                s.mouse_protocol_mode(),
                s.mouse_protocol_encoding(),
            )
        };
        if let Some(b) = bytes {
            p.write(&b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_clamp() {
        assert_eq!(clamp_sidebar(2, 100), MIN_SIDEBAR);
        assert_eq!(clamp_sidebar(30, 100), 30);
        assert_eq!(clamp_sidebar(80, 100), 50);
        assert_eq!(clamp_sidebar(15, 12), MIN_SIDEBAR);
    }
}

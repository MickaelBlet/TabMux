use crate::layout::{Node, PaneId};

pub struct Tab {
    /// Custom name set by the user; `None` follows the focused pane's title.
    pub name: Option<String>,
    pub root: Node,
    pub focus: PaneId,
    pub zoom: Option<PaneId>,
    /// Multicursor: keyboard input goes to every pane of the tab.
    pub sync: bool,
    /// A pane rang the bell while the tab was in the background.
    pub bell: bool,
    /// A pane produced output while the tab was in the background.
    pub activity: bool,
}

impl Tab {
    pub fn new(pane: PaneId) -> Self {
        Self {
            name: None,
            root: Node::Leaf(pane),
            focus: pane,
            zoom: None,
            sync: false,
            bell: false,
            activity: false,
        }
    }
}

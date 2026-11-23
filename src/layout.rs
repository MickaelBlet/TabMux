use ratatui::layout::Rect;

pub type PaneId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Children side by side, separated by a vertical line.
    Vertical,
    /// Children stacked, separated by a horizontal line.
    Horizontal,
}

#[derive(Debug, Clone)]
pub enum Node {
    Leaf(PaneId),
    Split {
        dir: Dir,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

/// Path from the root to a node: `false` = first child, `true` = second child.
pub type Path = Vec<bool>;

#[derive(Debug, Clone)]
pub struct Separator {
    pub rect: Rect,
    pub dir: Dir,
    pub parent: Rect,
    pub path: Path,
}

impl Separator {
    /// Part of the separator bordering `pane`, if they are adjacent.
    pub fn border(&self, pane: Rect) -> Option<Rect> {
        let r = self.rect;
        match self.dir {
            Dir::Vertical => {
                if pane.right() != r.x && pane.x != r.right() {
                    return None;
                }
                let top = r.top().max(pane.top());
                let bottom = r.bottom().min(pane.bottom());
                (top < bottom).then(|| Rect::new(r.x, top, r.width, bottom - top))
            }
            Dir::Horizontal => {
                if pane.bottom() != r.y && pane.y != r.bottom() {
                    return None;
                }
                let left = r.left().max(pane.left());
                let right = r.right().min(pane.right());
                (left < right).then(|| Rect::new(left, r.y, right - left, r.height))
            }
        }
    }
}

/// Where a dragged pane lands relative to the pane under the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Left,
    Right,
    Top,
    Bottom,
    /// Swap the two panes.
    Center,
}

impl Zone {
    /// Zone of `rect` under `pos`: the nearest edge, or the center when far
    /// from every edge.
    pub fn at(rect: Rect, x: u16, y: u16) -> Zone {
        let fx = (x.saturating_sub(rect.x) as f32 + 0.5) / rect.width.max(1) as f32;
        let fy = (y.saturating_sub(rect.y) as f32 + 0.5) / rect.height.max(1) as f32;
        let edges = [
            (fx, Zone::Left),
            (1.0 - fx, Zone::Right),
            (fy, Zone::Top),
            (1.0 - fy, Zone::Bottom),
        ];
        let (d, z) = edges
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((1.0, Zone::Center));
        if d > 0.25 { Zone::Center } else { z }
    }

    /// Part of `rect` covered by the dropped pane.
    pub fn rect(self, r: Rect) -> Rect {
        let (hw, hh) = (r.width / 2, r.height / 2);
        match self {
            Zone::Left => Rect { width: hw, ..r },
            Zone::Right => Rect {
                x: r.x + r.width - hw,
                width: hw,
                ..r
            },
            Zone::Top => Rect { height: hh, ..r },
            Zone::Bottom => Rect {
                y: r.y + r.height - hh,
                height: hh,
                ..r
            },
            Zone::Center => r,
        }
    }
}

pub const MIN_RATIO: f32 = 0.1;
pub const MAX_RATIO: f32 = 0.9;

impl Node {
    pub fn leaves(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<PaneId>) {
        match self {
            Node::Leaf(id) => out.push(*id),
            Node::Split { a, b, .. } => {
                a.collect(out);
                b.collect(out);
            }
        }
    }

    pub fn contains(&self, id: PaneId) -> bool {
        match self {
            Node::Leaf(x) => *x == id,
            Node::Split { a, b, .. } => a.contains(id) || b.contains(id),
        }
    }

    /// Replace leaf `target` with a split holding `target` and `new_id`.
    pub fn split(&mut self, target: PaneId, dir: Dir, new_id: PaneId) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a: Box::new(Node::Leaf(target)),
                    b: Box::new(Node::Leaf(new_id)),
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => {
                a.split(target, dir, new_id) || b.split(target, dir, new_id)
            }
        }
    }

    /// Replace leaf `target` with a split holding `target` and `new_id`,
    /// `new_id` placed on the side given by `zone` (not `Center`).
    pub fn insert(&mut self, target: PaneId, zone: Zone, new_id: PaneId) -> bool {
        let (dir, before) = match zone {
            Zone::Left => (Dir::Vertical, true),
            Zone::Right => (Dir::Vertical, false),
            Zone::Top => (Dir::Horizontal, true),
            Zone::Bottom => (Dir::Horizontal, false),
            Zone::Center => return false,
        };
        if !self.split(target, dir, new_id) {
            return false;
        }
        if before {
            self.swap(target, new_id);
        }
        true
    }

    /// Side of its sibling that leaf `id` occupies in its parent split, or
    /// `None` if `id` is the root or absent.
    pub fn side_of(&self, id: PaneId) -> Option<Zone> {
        let Node::Split { dir, a, b, .. } = self else {
            return None;
        };
        let side = |first: bool| match (dir, first) {
            (Dir::Vertical, true) => Zone::Left,
            (Dir::Vertical, false) => Zone::Right,
            (Dir::Horizontal, true) => Zone::Top,
            (Dir::Horizontal, false) => Zone::Bottom,
        };
        match (&**a, &**b) {
            (Node::Leaf(x), _) if *x == id => Some(side(true)),
            (_, Node::Leaf(x)) if *x == id => Some(side(false)),
            _ => a.side_of(id).or_else(|| b.side_of(id)),
        }
    }

    /// Exchange the positions of leaves `x` and `y`.
    pub fn swap(&mut self, x: PaneId, y: PaneId) {
        match self {
            Node::Leaf(id) if *id == x => *id = y,
            Node::Leaf(id) if *id == y => *id = x,
            Node::Leaf(_) => {}
            Node::Split { a, b, .. } => {
                a.swap(x, y);
                b.swap(x, y);
            }
        }
    }

    /// Move leaf `id` next to `target` (or swap them for `Zone::Center`).
    pub fn move_pane(self, id: PaneId, target: PaneId, zone: Zone) -> Node {
        if id == target || !self.contains(id) || !self.contains(target) {
            return self;
        }
        if zone == Zone::Center {
            let mut n = self;
            n.swap(id, target);
            return n;
        }
        let Some(mut n) = self.clone().remove(id) else {
            return self;
        };
        if n.insert(target, zone, id) { n } else { self }
    }

    /// Remove leaf `id`; its sibling takes the parent's place. Returns `None`
    /// if the tree becomes empty.
    pub fn remove(self, id: PaneId) -> Option<Node> {
        match self {
            Node::Leaf(x) if x == id => None,
            leaf @ Node::Leaf(_) => Some(leaf),
            Node::Split { dir, ratio, a, b } => match (a.remove(id), b.remove(id)) {
                (Some(a), Some(b)) => Some(Node::Split {
                    dir,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (Some(n), None) | (None, Some(n)) => Some(n),
                (None, None) => None,
            },
        }
    }

    pub fn set_ratio(&mut self, path: &[bool], ratio: f32) {
        match (self, path.split_first()) {
            (Node::Split { ratio: r, .. }, None) => *r = ratio.clamp(MIN_RATIO, MAX_RATIO),
            (Node::Split { a, b, .. }, Some((second, rest))) => {
                if *second {
                    b.set_ratio(rest, ratio)
                } else {
                    a.set_ratio(rest, ratio)
                }
            }
            _ => {}
        }
    }

    /// Compute pane rects and separators for `area`.
    pub fn compute(&self, area: Rect) -> (Vec<(PaneId, Rect)>, Vec<Separator>) {
        let mut panes = Vec::new();
        let mut seps = Vec::new();
        self.compute_into(area, &mut Vec::new(), &mut panes, &mut seps);
        (panes, seps)
    }

    fn compute_into(
        &self,
        area: Rect,
        path: &mut Path,
        panes: &mut Vec<(PaneId, Rect)>,
        seps: &mut Vec<Separator>,
    ) {
        match self {
            Node::Leaf(id) => panes.push((*id, area)),
            Node::Split { dir, ratio, a, b } => {
                let (ra, sep, rb) = split_rect(area, *dir, *ratio);
                if let Some(rect) = sep {
                    seps.push(Separator {
                        rect,
                        dir: *dir,
                        parent: area,
                        path: path.clone(),
                    });
                }
                path.push(false);
                a.compute_into(ra, path, panes, seps);
                path.pop();
                path.push(true);
                b.compute_into(rb, path, panes, seps);
                path.pop();
            }
        }
    }
}

/// Split `area` into (first, separator, second). The separator is one cell thick.
fn split_rect(area: Rect, dir: Dir, ratio: f32) -> (Rect, Option<Rect>, Rect) {
    let total = match dir {
        Dir::Vertical => area.width,
        Dir::Horizontal => area.height,
    };
    if total < 3 {
        let empty = Rect {
            width: 0,
            height: 0,
            ..area
        };
        return (area, None, empty);
    }
    let avail = total - 1;
    let first = ((avail as f32 * ratio).round() as u16).clamp(1, avail - 1);
    let second = avail - first;
    match dir {
        Dir::Vertical => (
            Rect {
                width: first,
                ..area
            },
            Some(Rect {
                x: area.x + first,
                width: 1,
                ..area
            }),
            Rect {
                x: area.x + first + 1,
                width: second,
                ..area
            },
        ),
        Dir::Horizontal => (
            Rect {
                height: first,
                ..area
            },
            Some(Rect {
                y: area.y + first,
                height: 1,
                ..area
            }),
            Rect {
                y: area.y + first + 1,
                height: second,
                ..area
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_of_leaf() {
        let mut n = Node::Leaf(1);
        assert_eq!(n.side_of(1), None);
        n.split(1, Dir::Vertical, 2);
        n.split(2, Dir::Horizontal, 3);
        assert_eq!(n.side_of(1), Some(Zone::Left));
        assert_eq!(n.side_of(2), Some(Zone::Top));
        assert_eq!(n.side_of(3), Some(Zone::Bottom));
        assert_eq!(n.side_of(4), None);
    }

    #[test]
    fn split_and_remove() {
        let mut n = Node::Leaf(1);
        assert!(n.split(1, Dir::Vertical, 2));
        assert!(n.split(2, Dir::Horizontal, 3));
        assert_eq!(n.leaves(), vec![1, 2, 3]);
        let n = n.remove(2).unwrap();
        assert_eq!(n.leaves(), vec![1, 3]);
        let n = n.remove(1).unwrap();
        assert!(matches!(n, Node::Leaf(3)));
        assert!(n.remove(3).is_none());
    }

    #[test]
    fn separator_border() {
        let mut n = Node::Leaf(1);
        n.split(1, Dir::Vertical, 2);
        n.split(2, Dir::Horizontal, 3);
        let (panes, seps) = n.compute(Rect::new(0, 0, 81, 25));
        let rect = |id| panes.iter().find(|(p, _)| *p == id).unwrap().1;
        let (v, h) = (&seps[0], &seps[1]);
        // Pane 1 borders the whole vertical separator, not the horizontal one.
        assert_eq!(v.border(rect(1)), Some(v.rect));
        assert_eq!(h.border(rect(1)), None);
        // Pane 3 borders the lower part of the vertical one and the horizontal one.
        let b = v.border(rect(3)).unwrap();
        assert_eq!((b.y, b.bottom()), (rect(3).y, rect(3).bottom()));
        assert_eq!(h.border(rect(3)), Some(h.rect));
    }

    #[test]
    fn compute_no_overlap() {
        let mut n = Node::Leaf(1);
        n.split(1, Dir::Vertical, 2);
        n.split(2, Dir::Horizontal, 3);
        let area = Rect::new(0, 0, 81, 25);
        let (panes, seps) = n.compute(area);
        assert_eq!(panes.len(), 3);
        assert_eq!(seps.len(), 2);
        let cells: u32 = panes.iter().map(|(_, r)| r.area()).sum::<u32>()
            + seps.iter().map(|s| s.rect.area()).sum::<u32>();
        assert_eq!(cells, area.area());
        for (i, (_, a)) in panes.iter().enumerate() {
            for (_, b) in &panes[i + 1..] {
                assert!(!a.intersects(*b));
            }
        }
        assert_eq!(panes[0].1, Rect::new(0, 0, 40, 25));
        assert_eq!(seps[0].rect, Rect::new(40, 0, 1, 25));
        assert_eq!(seps[1].path, vec![true]);
    }

    #[test]
    fn move_and_swap() {
        let mut n = Node::Leaf(1);
        n.split(1, Dir::Vertical, 2);
        n.split(2, Dir::Horizontal, 3);
        let n = n.move_pane(3, 1, Zone::Left);
        assert_eq!(n.leaves(), vec![3, 1, 2]);
        let n = n.move_pane(2, 3, Zone::Center);
        assert_eq!(n.leaves(), vec![2, 1, 3]);
        let n = n.move_pane(1, 1, Zone::Top);
        assert_eq!(n.leaves(), vec![2, 1, 3]);
    }

    #[test]
    fn zone_at() {
        let r = Rect::new(10, 10, 40, 20);
        assert_eq!(Zone::at(r, 11, 20), Zone::Left);
        assert_eq!(Zone::at(r, 48, 20), Zone::Right);
        assert_eq!(Zone::at(r, 30, 10), Zone::Top);
        assert_eq!(Zone::at(r, 30, 29), Zone::Bottom);
        assert_eq!(Zone::at(r, 30, 20), Zone::Center);
    }

    #[test]
    fn ratio_by_path() {
        let mut n = Node::Leaf(1);
        n.split(1, Dir::Vertical, 2);
        n.split(2, Dir::Horizontal, 3);
        n.set_ratio(&[true], 0.99);
        match &n {
            Node::Split { b, .. } => match **b {
                Node::Split { ratio, .. } => assert_eq!(ratio, MAX_RATIO),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }
}

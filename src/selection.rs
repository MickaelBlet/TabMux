use base64::Engine;

use crate::layout::PaneId;

/// A linear text selection inside a pane, in (row, col) content coordinates.
#[derive(Clone, Debug)]
pub struct Selection {
    pub pane: PaneId,
    pub anchor: (u16, u16),
    pub cursor: (u16, u16),
    /// Set by word/line selection: never empty, even when a single cell.
    pub word: bool,
}

impl Selection {
    pub fn new(pane: PaneId, pos: (u16, u16)) -> Self {
        Self {
            pane,
            anchor: pos,
            cursor: pos,
            word: false,
        }
    }

    pub fn ordered(&self) -> ((u16, u16), (u16, u16)) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor && !self.word
    }

    pub fn contains(&self, row: u16, col: u16) -> bool {
        let (start, end) = self.ordered();
        (row, col) >= start && (row, col) <= end
    }

    /// Selection covering the word (run of printable ASCII, no spaces) at `(row, col)`, if any.
    pub fn word_at(pane: PaneId, screen: &vt100::Screen, row: u16, col: u16) -> Option<Self> {
        let cols = screen.size().1;
        let is_word = |c: u16| {
            screen.cell(row, c).is_some_and(|cell| {
                let s = cell.contents();
                s.len() == 1 && s.as_bytes()[0].is_ascii_graphic()
            })
        };
        if col >= cols || !is_word(col) {
            return None;
        }
        let mut start = col;
        while start > 0 && is_word(start - 1) {
            start -= 1;
        }
        let mut end = col;
        while end + 1 < cols && is_word(end + 1) {
            end += 1;
        }
        Some(Self {
            pane,
            anchor: (row, start),
            cursor: (row, end),
            word: true,
        })
    }

    /// Selection covering screen row `row` up to its last non-blank cell, if any.
    pub fn line_at(pane: PaneId, screen: &vt100::Screen, row: u16) -> Option<Self> {
        let end = (0..screen.size().1).rev().find(|&c| {
            screen.cell(row, c).is_some_and(|cell| {
                cell.contents()
                    .chars()
                    .next()
                    .is_some_and(|ch| !ch.is_whitespace())
            })
        })?;
        Some(Self {
            pane,
            anchor: (row, 0),
            cursor: (row, end),
            word: true,
        })
    }

    pub fn text(&self, screen: &vt100::Screen) -> String {
        let (s, e) = self.ordered();
        screen.contents_between(s.0, s.1, e.0, e.1 + 1)
    }
}

/// Escape sequence asking the host terminal to put `text` in the clipboard.
pub fn osc52(text: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{b64}\x07")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_and_contains() {
        let mut s = Selection::new(1, (3, 5));
        s.cursor = (1, 2);
        assert_eq!(s.ordered(), ((1, 2), (3, 5)));
        assert!(s.contains(2, 0));
        assert!(s.contains(1, 2));
        assert!(s.contains(3, 5));
        assert!(!s.contains(1, 1));
        assert!(!s.contains(3, 6));
    }

    #[test]
    fn extracts_text() {
        let mut p = vt100::Parser::new(3, 10, 0);
        p.process(b"hello\r\nworld");
        let mut s = Selection::new(1, (0, 1));
        s.cursor = (1, 2);
        assert_eq!(s.text(p.screen()), "ello\nwor");
    }

    #[test]
    fn word_selection() {
        let mut p = vt100::Parser::new(3, 20, 0);
        p.process(b"ls  /tmp/a-b x");
        let s = Selection::word_at(1, p.screen(), 0, 6).unwrap();
        assert_eq!((s.anchor, s.cursor), ((0, 4), (0, 11)));
        assert_eq!(s.text(p.screen()), "/tmp/a-b");
        let s = Selection::word_at(1, p.screen(), 0, 13).unwrap();
        assert_eq!(s.text(p.screen()), "x");
        assert!(!s.is_empty());
        assert!(Selection::word_at(1, p.screen(), 0, 2).is_none());
        assert!(Selection::word_at(1, p.screen(), 0, 18).is_none());

        let mut p = vt100::Parser::new(3, 20, 0);
        p.process("é─foo│bar".as_bytes());
        let s = Selection::word_at(1, p.screen(), 0, 3).unwrap();
        assert_eq!(s.text(p.screen()), "foo");
        assert!(Selection::word_at(1, p.screen(), 0, 0).is_none());
    }

    #[test]
    fn line_selection() {
        let mut p = vt100::Parser::new(3, 20, 0);
        p.process(b"  ls -la   \r\nx");
        let s = Selection::line_at(1, p.screen(), 0).unwrap();
        assert_eq!((s.anchor, s.cursor), ((0, 0), (0, 7)));
        assert_eq!(s.text(p.screen()), "  ls -la");
        assert_eq!(
            Selection::line_at(1, p.screen(), 1)
                .unwrap()
                .text(p.screen()),
            "x"
        );
        assert!(Selection::line_at(1, p.screen(), 2).is_none());
    }

    #[test]
    fn osc52_encoding() {
        assert_eq!(osc52("hi"), "\x1b]52;c;aGk=\x07");
    }
}

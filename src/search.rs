//! Text search over a pane's full history (scrollback + screen).

use crate::layout::PaneId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    /// History line index, 0 = oldest line.
    pub line: usize,
    /// Column of the first matched cell.
    pub col: u16,
    /// Number of matched cells.
    pub len: u16,
}

pub struct Search {
    pub pane: PaneId,
    pub query: String,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    /// Matches must be recomputed (query edited or new output).
    pub dirty: bool,
}

impl Search {
    pub fn new(pane: PaneId) -> Self {
        Self {
            pane,
            query: String::new(),
            matches: Vec::new(),
            current: None,
            dirty: true,
        }
    }

    pub fn current_match(&self) -> Option<Match> {
        self.current.and_then(|i| self.matches.get(i).copied())
    }

    /// Replace matches, keeping the current one on the same text if possible;
    /// otherwise select the most recent match.
    pub fn set_matches(&mut self, matches: Vec<Match>, keep: bool) {
        let prev = self.current_match().filter(|_| keep);
        self.matches = matches;
        self.current = match prev {
            Some(p) => self
                .matches
                .iter()
                .position(|m| (m.line, m.col) >= (p.line, p.col))
                .or(self.matches.len().checked_sub(1)),
            None => self.matches.len().checked_sub(1),
        };
    }

    /// Move to an older (`-1`) or newer (`+1`) match, wrapping around.
    pub fn step(&mut self, delta: isize) {
        let n = self.matches.len() as isize;
        if n == 0 {
            return;
        }
        let cur = self.current.map_or(n - 1, |c| c as isize);
        self.current = Some((cur + delta).rem_euclid(n) as usize);
    }
}

/// Find `query` in `lines`. Smart case: case-insensitive unless the query
/// contains an uppercase letter. Columns are counted in chars.
pub fn find(lines: &[String], query: &str) -> Vec<Match> {
    if query.is_empty() {
        return Vec::new();
    }
    let sensitive = query.chars().any(char::is_uppercase);
    let norm = |s: &str| -> Vec<char> {
        if sensitive {
            s.chars().collect()
        } else {
            s.chars().flat_map(char::to_lowercase).collect()
        }
    };
    let q = norm(query);
    let mut out = Vec::new();
    for (line, text) in lines.iter().enumerate() {
        let t = norm(text);
        if t.len() < q.len() {
            continue;
        }
        let mut i = 0;
        while i + q.len() <= t.len() {
            if t[i..i + q.len()] == q[..] {
                out.push(Match {
                    line,
                    col: i as u16,
                    len: q.len() as u16,
                });
                i += q.len();
            } else {
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn smart_case() {
        let l = lines(&["Error here", "no error", "ERROR"]);
        assert_eq!(find(&l, "error").len(), 3);
        let m = find(&l, "Error");
        assert_eq!(
            m,
            vec![Match {
                line: 0,
                col: 0,
                len: 5
            }]
        );
    }

    #[test]
    fn multiple_per_line_and_columns() {
        let l = lines(&["ab ab", "xab"]);
        let m = find(&l, "ab");
        let pos: Vec<_> = m.iter().map(|m| (m.line, m.col)).collect();
        assert_eq!(pos, vec![(0, 0), (0, 3), (1, 1)]);
        assert!(find(&l, "").is_empty());
    }

    #[test]
    fn step_wraps_and_defaults_to_latest() {
        let mut s = Search::new(1);
        s.set_matches(find(&lines(&["a", "a", "a"]), "a"), false);
        assert_eq!(s.current, Some(2));
        s.step(1);
        assert_eq!(s.current, Some(0));
        s.step(-1);
        assert_eq!(s.current, Some(2));
    }
}

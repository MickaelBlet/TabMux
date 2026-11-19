//! Prompt marks from shell integration (OSC 133;A), used to jump between commands.
//!
//! Marks are stored as absolute line numbers: lines evicted from the scrollback
//! plus the history index. History index 0 is the oldest line still kept.

#[derive(Default, Debug)]
pub struct Marks {
    lines: Vec<u64>,
    /// Lines dropped from the front of the scrollback since the pane started.
    evicted: u64,
}

impl Marks {
    /// Record a prompt at `index` (history index). Marks at or below it are dropped:
    /// the screen was cleared or redrawn over them.
    pub fn add(&mut self, index: usize) {
        let abs = self.evicted + index as u64;
        self.lines.retain(|&l| l < abs);
        self.lines.push(abs);
    }

    /// One line left the scrollback.
    pub fn evict(&mut self) {
        self.evicted += 1;
        let e = self.evicted;
        if self.lines.first().is_some_and(|&l| l < e) {
            self.lines.retain(|&l| l >= e);
        }
    }

    /// Marks as history indices, oldest first.
    pub fn indices(&self) -> Vec<usize> {
        self.lines
            .iter()
            .map(|&l| (l - self.evicted) as usize)
            .collect()
    }
}

/// Closest mark before (`delta < 0`) or after (`delta > 0`) `from`.
pub fn step(marks: &[usize], from: usize, delta: isize) -> Option<usize> {
    if delta < 0 {
        marks.iter().rev().find(|&&m| m < from).copied()
    } else {
        marks.iter().find(|&&m| m > from).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_drops_overwritten_marks() {
        let mut m = Marks::default();
        m.add(3);
        m.add(10);
        m.add(20);
        m.add(10); // screen cleared, prompt redrawn higher up
        assert_eq!(m.indices(), vec![3, 10]);
    }

    #[test]
    fn evict_shifts_and_prunes() {
        let mut m = Marks::default();
        m.add(0);
        m.add(5);
        m.evict();
        assert_eq!(m.indices(), vec![4]);
        m.add(7);
        assert_eq!(m.indices(), vec![4, 7]);
    }

    #[test]
    fn step_prev_next() {
        let marks = [2, 8, 15];
        assert_eq!(step(&marks, 15, -1), Some(8));
        assert_eq!(step(&marks, 16, -1), Some(15));
        assert_eq!(step(&marks, 2, -1), None);
        assert_eq!(step(&marks, 8, 1), Some(15));
        assert_eq!(step(&marks, 15, 1), None);
    }
}

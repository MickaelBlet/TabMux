use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Encode a key event into the byte sequence a terminal application expects.
pub fn encode_key(key: KeyEvent, app_cursor: bool) -> Vec<u8> {
    let m = key.modifiers;
    let ctrl = m.contains(KeyModifiers::CONTROL);
    let alt = m.contains(KeyModifiers::ALT);
    let shift = m.contains(KeyModifiers::SHIFT);
    // xterm modifier parameter: 1 + shift(1) + alt(2) + ctrl(4)
    let mod_param = 1 + shift as u8 + 2 * alt as u8 + 4 * ctrl as u8;

    let csi_letter = |c: char| -> Vec<u8> {
        if mod_param > 1 {
            format!("\x1b[1;{mod_param}{c}").into_bytes()
        } else if app_cursor {
            format!("\x1bO{c}").into_bytes()
        } else {
            format!("\x1b[{c}").into_bytes()
        }
    };
    let csi_tilde = |n: u8| -> Vec<u8> {
        if mod_param > 1 {
            format!("\x1b[{n};{mod_param}~").into_bytes()
        } else {
            format!("\x1b[{n}~").into_bytes()
        }
    };

    let mut out = match key.code {
        KeyCode::Char(c) if ctrl => match ctrl_byte(c) {
            Some(b) => vec![b],
            None => c.to_string().into_bytes(),
        },
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![if ctrl { 0x08 } else { 0x7f }],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => return csi_letter('A'),
        KeyCode::Down => return csi_letter('B'),
        KeyCode::Right => return csi_letter('C'),
        KeyCode::Left => return csi_letter('D'),
        KeyCode::Home => return csi_letter('H'),
        KeyCode::End => return csi_letter('F'),
        KeyCode::Insert => return csi_tilde(2),
        KeyCode::Delete => return csi_tilde(3),
        KeyCode::PageUp => return csi_tilde(5),
        KeyCode::PageDown => return csi_tilde(6),
        KeyCode::F(n @ 1..=4) => {
            let c = (b'P' + n - 1) as char;
            return if mod_param > 1 {
                format!("\x1b[1;{mod_param}{c}").into_bytes()
            } else {
                format!("\x1bO{c}").into_bytes()
            };
        }
        KeyCode::F(n @ 5..=12) => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            return csi_tilde(CODES[(n - 5) as usize]);
        }
        _ => return Vec::new(),
    };
    if alt {
        out.insert(0, 0x1b);
    }
    out
}

fn ctrl_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        '@' | ' ' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, m)
    }

    #[test]
    fn plain_and_ctrl_chars() {
        assert_eq!(
            encode_key(k(KeyCode::Char('a'), KeyModifiers::NONE), false),
            b"a"
        );
        assert_eq!(
            encode_key(k(KeyCode::Char('é'), KeyModifiers::NONE), false),
            "é".as_bytes()
        );
        assert_eq!(
            encode_key(k(KeyCode::Char('c'), KeyModifiers::CONTROL), false),
            [3]
        );
        assert_eq!(
            encode_key(k(KeyCode::Char('x'), KeyModifiers::ALT), false),
            b"\x1bx"
        );
    }

    #[test]
    fn arrows() {
        assert_eq!(
            encode_key(k(KeyCode::Up, KeyModifiers::NONE), false),
            b"\x1b[A"
        );
        assert_eq!(
            encode_key(k(KeyCode::Up, KeyModifiers::NONE), true),
            b"\x1bOA"
        );
        assert_eq!(
            encode_key(k(KeyCode::Left, KeyModifiers::CONTROL), false),
            b"\x1b[1;5D"
        );
    }

    #[test]
    fn special_keys() {
        assert_eq!(
            encode_key(k(KeyCode::Enter, KeyModifiers::NONE), false),
            b"\r"
        );
        assert_eq!(
            encode_key(k(KeyCode::Backspace, KeyModifiers::NONE), false),
            [0x7f]
        );
        assert_eq!(
            encode_key(k(KeyCode::Delete, KeyModifiers::NONE), false),
            b"\x1b[3~"
        );
        assert_eq!(
            encode_key(k(KeyCode::F(1), KeyModifiers::NONE), false),
            b"\x1bOP"
        );
        assert_eq!(
            encode_key(k(KeyCode::F(5), KeyModifiers::NONE), false),
            b"\x1b[15~"
        );
    }
}

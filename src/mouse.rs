use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
use vt100::{MouseProtocolEncoding, MouseProtocolMode};

fn button_code(b: MouseButton) -> u8 {
    match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

/// Encode a mouse event for an application running in a PTY.
/// `col`/`row` are 0-based, relative to the pane content.
/// Returns `None` if the application's mouse mode doesn't want this event.
pub fn encode(
    kind: MouseEventKind,
    mods: KeyModifiers,
    col: u16,
    row: u16,
    mode: MouseProtocolMode,
    enc: MouseProtocolEncoding,
) -> Option<Vec<u8>> {
    use MouseProtocolMode as M;
    let (mut code, release) = match kind {
        MouseEventKind::Down(b) => (button_code(b), false),
        MouseEventKind::Up(b) => {
            if mode == M::Press {
                return None;
            }
            (button_code(b), true)
        }
        MouseEventKind::Drag(b) => {
            if !matches!(mode, M::ButtonMotion | M::AnyMotion) {
                return None;
            }
            (button_code(b) + 32, false)
        }
        MouseEventKind::Moved => {
            if mode != M::AnyMotion {
                return None;
            }
            (3 + 32, false)
        }
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        _ => return None,
    };
    if mode == M::None {
        return None;
    }
    if mods.contains(KeyModifiers::SHIFT) {
        code += 4;
    }
    if mods.contains(KeyModifiers::ALT) {
        code += 8;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        code += 16;
    }
    let (x, y) = (u32::from(col) + 1, u32::from(row) + 1);

    match enc {
        MouseProtocolEncoding::Sgr => {
            let end = if release { 'm' } else { 'M' };
            Some(format!("\x1b[<{code};{x};{y}{end}").into_bytes())
        }
        MouseProtocolEncoding::Default | MouseProtocolEncoding::Utf8 => {
            // Legacy encodings cannot report which button was released.
            let code = if release { 3 | (code & !3) } else { code };
            let mut out = b"\x1b[M".to_vec();
            out.push(32 + code);
            for v in [x, y] {
                let v = v + 32;
                if enc == MouseProtocolEncoding::Default {
                    out.push(v.min(255) as u8);
                } else {
                    let mut buf = [0u8; 4];
                    let c = char::from_u32(v)?;
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
            }
            Some(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyModifiers = KeyModifiers::NONE;

    #[test]
    fn sgr() {
        let e = |k| {
            encode(
                k,
                NONE,
                4,
                9,
                MouseProtocolMode::ButtonMotion,
                MouseProtocolEncoding::Sgr,
            )
        };
        assert_eq!(
            e(MouseEventKind::Down(MouseButton::Left)).unwrap(),
            b"\x1b[<0;5;10M"
        );
        assert_eq!(
            e(MouseEventKind::Up(MouseButton::Right)).unwrap(),
            b"\x1b[<2;5;10m"
        );
        assert_eq!(
            e(MouseEventKind::Drag(MouseButton::Left)).unwrap(),
            b"\x1b[<32;5;10M"
        );
        assert_eq!(e(MouseEventKind::ScrollDown).unwrap(), b"\x1b[<65;5;10M");
        assert!(e(MouseEventKind::Moved).is_none());
    }

    #[test]
    fn legacy() {
        let e = |k| {
            encode(
                k,
                NONE,
                0,
                0,
                MouseProtocolMode::PressRelease,
                MouseProtocolEncoding::Default,
            )
        };
        assert_eq!(
            e(MouseEventKind::Down(MouseButton::Left)).unwrap(),
            b"\x1b[M !!"
        );
        assert_eq!(
            e(MouseEventKind::Up(MouseButton::Left)).unwrap(),
            b"\x1b[M#!!"
        );
        assert!(e(MouseEventKind::Drag(MouseButton::Left)).is_none());
    }

    #[test]
    fn disabled() {
        let r = encode(
            MouseEventKind::Down(MouseButton::Left),
            NONE,
            0,
            0,
            MouseProtocolMode::None,
            MouseProtocolEncoding::Sgr,
        );
        assert!(r.is_none());
    }
}

//! Key → byte-sequence encoding for the mobile keyboard panel and hardware
//! keyboards. Lives in Rust so the xterm conventions (application cursor
//! mode, modifier parameters, control codes) are in one tested place and
//! Kotlin only says *which* key was pressed.

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SpecialKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, uniffi::Record)]
pub struct KeyMods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyMods {
    /// xterm modifier parameter: `1 + shift·1 + alt·2 + ctrl·4`, or `None`
    /// when nothing is held.
    fn param(self) -> Option<u8> {
        let v = 1 + u8::from(self.shift) + (u8::from(self.alt) << 1) + (u8::from(self.ctrl) << 2);
        (v > 1).then_some(v)
    }
}

/// Bytes for a special key under the given modifiers. `app_cursor` is the
/// terminal's DECCKM state (arrows/Home/End become SS3 sequences).
pub fn encode_key(key: SpecialKey, mods: KeyMods, app_cursor: bool) -> Vec<u8> {
    use SpecialKey::*;
    let param = mods.param();
    let csi = |final_byte: char| -> Vec<u8> {
        match param {
            Some(m) => format!("\x1b[1;{m}{final_byte}").into_bytes(),
            None if app_cursor => format!("\x1bO{final_byte}").into_bytes(),
            None => format!("\x1b[{final_byte}").into_bytes(),
        }
    };
    let tilde = |code: u8| -> Vec<u8> {
        match param {
            Some(m) => format!("\x1b[{code};{m}~").into_bytes(),
            None => format!("\x1b[{code}~").into_bytes(),
        }
    };
    let mut out = match key {
        Enter => vec![b'\r'],
        Tab if mods.shift => b"\x1b[Z".to_vec(),
        Tab => vec![b'\t'],
        Backspace if mods.ctrl => vec![0x08],
        Backspace => vec![0x7f],
        Escape => vec![0x1b],
        Up => csi('A'),
        Down => csi('B'),
        Right => csi('C'),
        Left => csi('D'),
        Home => csi('H'),
        End => csi('F'),
        Insert => tilde(2),
        Delete => tilde(3),
        PageUp => tilde(5),
        PageDown => tilde(6),
        F1 | F2 | F3 | F4 => {
            let final_byte = match key {
                F1 => 'P',
                F2 => 'Q',
                F3 => 'R',
                _ => 'S',
            };
            match param {
                Some(m) => format!("\x1b[1;{m}{final_byte}").into_bytes(),
                None => format!("\x1bO{final_byte}").into_bytes(),
            }
        }
        F5 => tilde(15),
        F6 => tilde(17),
        F7 => tilde(18),
        F8 => tilde(19),
        F9 => tilde(20),
        F10 => tilde(21),
        F11 => tilde(23),
        F12 => tilde(24),
    };
    // Alt on keys that have no modifier parameter of their own: ESC prefix.
    if mods.alt && matches!(key, Enter | Tab | Backspace | Escape) {
        out.insert(0, 0x1b);
    }
    out
}

/// Bytes for typed text under modifiers: Ctrl turns letters and the usual
/// punctuation into control codes, Alt prefixes each character with ESC.
/// Shift is already reflected in the text the IME delivers.
pub fn encode_text(text: &str, mods: KeyMods) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 2);
    for ch in text.chars() {
        if mods.alt {
            out.push(0x1b);
        }
        if mods.ctrl
            && let Some(code) = control_code(ch)
        {
            out.push(code);
            continue;
        }
        let mut buf = [0u8; 4];
        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
    }
    out
}

/// The C0 byte a terminal sends for Ctrl+`ch`, if there is one.
pub fn control_code(ch: char) -> Option<u8> {
    match ch {
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '@' | ' ' | '2' => Some(0x00),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '7' | '-' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: KeyMods = KeyMods {
        ctrl: false,
        alt: false,
        shift: false,
    };
    const CTRL: KeyMods = KeyMods {
        ctrl: true,
        alt: false,
        shift: false,
    };
    const ALT: KeyMods = KeyMods {
        ctrl: false,
        alt: true,
        shift: false,
    };
    const SHIFT: KeyMods = KeyMods {
        ctrl: false,
        alt: false,
        shift: true,
    };

    #[test]
    fn arrows_follow_cursor_mode() {
        assert_eq!(encode_key(SpecialKey::Up, NONE, false), b"\x1b[A");
        assert_eq!(encode_key(SpecialKey::Up, NONE, true), b"\x1bOA");
        assert_eq!(encode_key(SpecialKey::Left, CTRL, true), b"\x1b[1;5D");
        assert_eq!(encode_key(SpecialKey::Right, ALT, false), b"\x1b[1;3C");
        assert_eq!(
            encode_key(
                SpecialKey::Down,
                KeyMods {
                    ctrl: true,
                    alt: false,
                    shift: true
                },
                false
            ),
            b"\x1b[1;6B"
        );
    }

    #[test]
    fn editing_and_function_keys() {
        assert_eq!(encode_key(SpecialKey::Delete, NONE, false), b"\x1b[3~");
        assert_eq!(encode_key(SpecialKey::PageUp, SHIFT, false), b"\x1b[5;2~");
        assert_eq!(encode_key(SpecialKey::Home, NONE, false), b"\x1b[H");
        assert_eq!(encode_key(SpecialKey::F1, NONE, false), b"\x1bOP");
        assert_eq!(encode_key(SpecialKey::F5, NONE, false), b"\x1b[15~");
        assert_eq!(encode_key(SpecialKey::F12, CTRL, false), b"\x1b[24;5~");
    }

    #[test]
    fn simple_keys_and_alt_prefix() {
        assert_eq!(encode_key(SpecialKey::Enter, NONE, false), b"\r");
        assert_eq!(encode_key(SpecialKey::Tab, SHIFT, false), b"\x1b[Z");
        assert_eq!(encode_key(SpecialKey::Backspace, NONE, false), [0x7f]);
        assert_eq!(encode_key(SpecialKey::Backspace, CTRL, false), [0x08]);
        assert_eq!(encode_key(SpecialKey::Escape, ALT, false), b"\x1b\x1b");
        assert_eq!(encode_key(SpecialKey::Enter, ALT, false), b"\x1b\r");
    }

    #[test]
    fn control_codes_for_text() {
        assert_eq!(encode_text("c", CTRL), [0x03]);
        assert_eq!(encode_text("C", CTRL), [0x03]);
        assert_eq!(encode_text("[", CTRL), [0x1b]);
        assert_eq!(encode_text(" ", CTRL), [0x00]);
        assert_eq!(encode_text("?", CTRL), [0x7f]);
        // No control code: fall through to plain text.
        assert_eq!(encode_text("é", CTRL), "é".as_bytes());
        assert_eq!(encode_text("x", ALT), b"\x1bx");
        assert_eq!(encode_text("ab", NONE), b"ab");
        assert_eq!(encode_text("日", NONE), "日".as_bytes());
    }
}

//! Terminal emulation for the mobile shell: `alacritty_terminal` parses the
//! byte stream and keeps the grid; Kotlin only renders flat snapshots.
//!
//! A snapshot is column-major-free: `cols × rows` cells, each a code point,
//! an RGB foreground/background already resolved through the palette, and a
//! bit set of attributes — cheap to move across the FFI and to draw on a
//! Canvas without knowing anything about ANSI.

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Processor, Rgb};
use tokio::sync::mpsc;

/// Cell attribute bits in [`GridSnapshot::flags`].
pub mod flag {
    pub const BOLD: u8 = 1 << 0;
    pub const ITALIC: u8 = 1 << 1;
    pub const UNDERLINE: u8 = 1 << 2;
    pub const STRIKEOUT: u8 = 1 << 3;
    pub const DIM: u8 = 1 << 4;
    /// Double-width character; the next cell is its spacer.
    pub const WIDE: u8 = 1 << 5;
    /// Second half of a wide character: draw nothing.
    pub const WIDE_SPACER: u8 = 1 << 6;
    pub const HIDDEN: u8 = 1 << 7;
}

/// Colours the emulator resolves named/indexed colours against. All values
/// are `0xRRGGBB`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TerminalPalette {
    pub foreground: u32,
    pub background: u32,
    pub cursor: u32,
    /// The 16 ANSI colours (normal 0–7, bright 8–15).
    pub ansi: Vec<u32>,
}

impl TerminalPalette {
    /// Termoso Dark.
    pub fn termoso_dark() -> Self {
        Self {
            foreground: 0xF1F3F8,
            background: 0x141826,
            cursor: 0x2BB884,
            ansi: vec![
                0x1A1E2B, 0xF25E61, 0x2BB884, 0xF2C94C, 0x5AA9E6, 0xC58AF9, 0x4FD1C5, 0xC9CDDA,
                0x5A6076, 0xFF8285, 0x5FD0A4, 0xFFDB7A, 0x8CC5F0, 0xDBB3FF, 0x84E7DD, 0xF2F4FA,
            ],
        }
    }

    fn ansi(&self, i: usize) -> u32 {
        self.ansi.get(i).copied().unwrap_or(self.foreground)
    }

    fn indexed(&self, i: u8) -> u32 {
        match i {
            0..=15 => self.ansi(i as usize),
            16..=231 => {
                let i = i as u32 - 16;
                let step = |v: u32| if v == 0 { 0 } else { 55 + v * 40 };
                let r = step(i / 36);
                let g = step((i / 6) % 6);
                let b = step(i % 6);
                (r << 16) | (g << 8) | b
            }
            232..=255 => {
                let v = 8 + (i as u32 - 232) * 10;
                (v << 16) | (v << 8) | v
            }
        }
    }

    fn named(&self, c: NamedColor) -> u32 {
        match c {
            NamedColor::Black => self.ansi(0),
            NamedColor::Red => self.ansi(1),
            NamedColor::Green => self.ansi(2),
            NamedColor::Yellow => self.ansi(3),
            NamedColor::Blue => self.ansi(4),
            NamedColor::Magenta => self.ansi(5),
            NamedColor::Cyan => self.ansi(6),
            NamedColor::White => self.ansi(7),
            NamedColor::BrightBlack => self.ansi(8),
            NamedColor::BrightRed => self.ansi(9),
            NamedColor::BrightGreen => self.ansi(10),
            NamedColor::BrightYellow => self.ansi(11),
            NamedColor::BrightBlue => self.ansi(12),
            NamedColor::BrightMagenta => self.ansi(13),
            NamedColor::BrightCyan => self.ansi(14),
            NamedColor::BrightWhite => self.ansi(15),
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::DimForeground => dim(self.foreground),
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimBlack => dim(self.ansi(0)),
            NamedColor::DimRed => dim(self.ansi(1)),
            NamedColor::DimGreen => dim(self.ansi(2)),
            NamedColor::DimYellow => dim(self.ansi(3)),
            NamedColor::DimBlue => dim(self.ansi(4)),
            NamedColor::DimMagenta => dim(self.ansi(5)),
            NamedColor::DimCyan => dim(self.ansi(6)),
            NamedColor::DimWhite => dim(self.ansi(7)),
        }
    }
}

fn dim(c: u32) -> u32 {
    let f = |v: u32| (v * 2 / 3) & 0xFF;
    (f((c >> 16) & 0xFF) << 16) | (f((c >> 8) & 0xFF) << 8) | f(c & 0xFF)
}

fn rgb(c: Rgb) -> u32 {
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
    HollowBlock,
    Hidden,
}

/// Everything needed to paint one frame.
#[derive(Debug, Clone, uniffi::Record)]
pub struct GridSnapshot {
    pub cols: u16,
    pub rows: u16,
    /// Row-major, `cols × rows` entries each: Unicode scalar per cell
    /// (0 = blank / wide spacer), resolved `0xRRGGBB` colours, attribute
    /// bits from [`flag`].
    pub chars: Vec<u32>,
    pub fg: Vec<u32>,
    pub bg: Vec<u32>,
    pub flags: Vec<u8>,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor: CursorStyle,
    /// Lines scrolled back into history (0 = live view).
    pub display_offset: u32,
    /// Scrollback lines available.
    pub history: u32,
    pub background: u32,
    /// Application asked for mouse reports (touch → mouse events).
    pub mouse_reporting: bool,
    /// Alternate screen active (no scrollback; wheel → arrow keys).
    pub alt_screen: bool,
    /// Bracketed paste mode enabled.
    pub bracketed_paste: bool,
    /// Application cursor keys mode.
    pub app_cursor: bool,
}

/// Bytes per cell in [`GridFrame::cells`].
pub const CELL_BYTES: usize = 12;

/// [`GridSnapshot`] packed for the FFI: one byte array instead of three
/// boxed lists, so a frame costs a single copy on the Kotlin side.
///
/// `cells` holds `cols × rows` little-endian triples of `u32`:
/// `[code point, flags << 24 | fg RGB, bg RGB]`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct GridFrame {
    pub cols: u16,
    pub rows: u16,
    pub cells: Vec<u8>,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor: CursorStyle,
    pub display_offset: u32,
    pub history: u32,
    pub background: u32,
    pub mouse_reporting: bool,
    pub alt_screen: bool,
    pub bracketed_paste: bool,
    pub app_cursor: bool,
}

impl GridSnapshot {
    pub fn pack(self) -> GridFrame {
        let n = self.chars.len();
        let mut cells = Vec::with_capacity(n * CELL_BYTES);
        for i in 0..n {
            cells.extend_from_slice(&self.chars[i].to_le_bytes());
            let fg = (self.fg[i] & 0x00FF_FFFF) | ((self.flags[i] as u32) << 24);
            cells.extend_from_slice(&fg.to_le_bytes());
            cells.extend_from_slice(&(self.bg[i] & 0x00FF_FFFF).to_le_bytes());
        }
        GridFrame {
            cols: self.cols,
            rows: self.rows,
            cells,
            cursor_col: self.cursor_col,
            cursor_row: self.cursor_row,
            cursor: self.cursor,
            display_offset: self.display_offset,
            history: self.history,
            background: self.background,
            mouse_reporting: self.mouse_reporting,
            alt_screen: self.alt_screen,
            bracketed_paste: self.bracketed_paste,
            app_cursor: self.app_cursor,
        }
    }
}

/// Side effects the emulator raises while parsing.
#[derive(Debug)]
pub enum TermSignal {
    /// Reply to write back to the remote (DA, DSR, colour queries…).
    PtyWrite(Vec<u8>),
    Title(Option<String>),
    Bell,
    /// OSC 52 copy request.
    Clipboard(String),
}

struct Proxy {
    tx: mpsc::UnboundedSender<TermSignal>,
    size: Arc<Mutex<(u16, u16)>>,
}

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        let signal = match event {
            Event::PtyWrite(s) => TermSignal::PtyWrite(s.into_bytes()),
            Event::Title(t) => TermSignal::Title(Some(t)),
            Event::ResetTitle => TermSignal::Title(None),
            Event::Bell => TermSignal::Bell,
            Event::ClipboardStore(_, text) => TermSignal::Clipboard(text),
            Event::ColorRequest(index, fmt) => {
                // Answer with our palette so `tput`-style probes do not hang.
                let c = default_color(index);
                TermSignal::PtyWrite(fmt(c).into_bytes())
            }
            Event::TextAreaSizeRequest(fmt) => {
                let (cols, rows) = *self.size.lock().expect("size poisoned");
                let size = alacritty_terminal::event::WindowSize {
                    num_lines: rows,
                    num_cols: cols,
                    cell_width: 1,
                    cell_height: 1,
                };
                TermSignal::PtyWrite(fmt(size).into_bytes())
            }
            _ => return,
        };
        let _ = self.tx.send(signal);
    }
}

fn default_color(index: usize) -> Rgb {
    let p = TerminalPalette::termoso_dark();
    let c = if index < 256 {
        p.indexed(index as u8)
    } else if index == NamedColor::Background as usize {
        p.background
    } else if index == NamedColor::Cursor as usize {
        p.cursor
    } else {
        p.foreground
    };
    Rgb {
        r: (c >> 16) as u8,
        g: (c >> 8) as u8,
        b: c as u8,
    }
}

#[derive(Clone, Copy)]
struct Size {
    cols: u16,
    rows: u16,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// The emulator: feed bytes, take snapshots.
pub struct Emulator {
    term: Term<Proxy>,
    parser: Processor,
    palette: TerminalPalette,
    size: Size,
    proxy_size: Arc<Mutex<(u16, u16)>>,
}

impl Emulator {
    pub fn new(
        cols: u16,
        rows: u16,
        scrollback: u32,
        palette: TerminalPalette,
    ) -> (Self, mpsc::UnboundedReceiver<TermSignal>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let size = Size {
            cols: cols.max(2),
            rows: rows.max(1),
        };
        let proxy_size = Arc::new(Mutex::new((size.cols, size.rows)));
        let proxy = Proxy {
            tx,
            size: proxy_size.clone(),
        };
        let config = Config {
            scrolling_history: scrollback as usize,
            ..Config::default()
        };
        let term = Term::new(config, &size, proxy);
        (
            Self {
                term,
                parser: Processor::new(),
                palette,
                size,
                proxy_size,
            },
            rx,
        )
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let size = Size {
            cols: cols.max(2),
            rows: rows.max(1),
        };
        if size.cols == self.size.cols && size.rows == self.size.rows {
            return;
        }
        self.size = size;
        *self.proxy_size.lock().expect("size poisoned") = (size.cols, size.rows);
        self.term.resize(size);
    }

    pub fn set_palette(&mut self, palette: TerminalPalette) {
        self.palette = palette;
    }

    pub fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }

    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    /// Text of the visible screen, one line per row (trailing blanks
    /// trimmed).
    pub fn visible_text(&self) -> Vec<String> {
        let content = self.term.renderable_content();
        let rows = self.size.rows as usize;
        let mut lines = vec![String::new(); rows];
        for cell in content.display_iter {
            let row = (cell.point.line.0 + content.display_offset as i32) as usize;
            if row >= rows || cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            lines[row].push(cell.c);
        }
        for l in &mut lines {
            let trimmed = l.trim_end().len();
            l.truncate(trimmed);
        }
        lines
    }

    pub fn snapshot(&self) -> GridSnapshot {
        let cols = self.size.cols as usize;
        let rows = self.size.rows as usize;
        let n = cols * rows;
        let mut chars = vec![0u32; n];
        let mut fg = vec![self.palette.foreground; n];
        let mut bg = vec![self.palette.background; n];
        let mut flags = vec![0u8; n];

        let content = self.term.renderable_content();
        let display_offset = content.display_offset;
        let colors = content.colors;
        let resolve = |c: Color| -> u32 {
            match c {
                Color::Spec(rgb_) => rgb(rgb_),
                Color::Indexed(i) => colors[i as usize]
                    .map(rgb)
                    .unwrap_or_else(|| self.palette.indexed(i)),
                Color::Named(named) => colors[named]
                    .map(rgb)
                    .unwrap_or_else(|| self.palette.named(named)),
            }
        };

        for cell in content.display_iter {
            let row = cell.point.line.0 + display_offset as i32;
            if row < 0 || row as usize >= rows {
                continue;
            }
            let col = cell.point.column.0;
            if col >= cols {
                continue;
            }
            let idx = row as usize * cols + col;
            let f = cell.flags;
            let mut bits = 0u8;
            if f.intersects(Flags::BOLD) {
                bits |= flag::BOLD;
            }
            if f.intersects(Flags::ITALIC) {
                bits |= flag::ITALIC;
            }
            if f.intersects(Flags::ALL_UNDERLINES) {
                bits |= flag::UNDERLINE;
            }
            if f.contains(Flags::STRIKEOUT) {
                bits |= flag::STRIKEOUT;
            }
            if f.intersects(Flags::DIM) {
                bits |= flag::DIM;
            }
            if f.contains(Flags::WIDE_CHAR) {
                bits |= flag::WIDE;
            }
            if f.contains(Flags::WIDE_CHAR_SPACER) || f.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                bits |= flag::WIDE_SPACER;
            }
            if f.contains(Flags::HIDDEN) {
                bits |= flag::HIDDEN;
            }
            let (mut cfg, mut cbg) = (resolve(cell.fg), resolve(cell.bg));
            if f.contains(Flags::INVERSE) {
                std::mem::swap(&mut cfg, &mut cbg);
            }
            if bits & flag::DIM != 0 {
                cfg = dim(cfg);
            }
            chars[idx] = if bits & flag::WIDE_SPACER != 0 {
                0
            } else {
                cell.c as u32
            };
            fg[idx] = cfg;
            bg[idx] = cbg;
            flags[idx] = bits;
        }

        let cursor_row = content.cursor.point.line.0 + display_offset as i32;
        let cursor_visible = (0..rows as i32).contains(&cursor_row);
        let cursor = if !cursor_visible {
            CursorStyle::Hidden
        } else {
            match content.cursor.shape {
                CursorShape::Block => CursorStyle::Block,
                CursorShape::Underline => CursorStyle::Underline,
                CursorShape::Beam => CursorStyle::Beam,
                CursorShape::HollowBlock => CursorStyle::HollowBlock,
                CursorShape::Hidden => CursorStyle::Hidden,
            }
        };
        let mode = content.mode;

        GridSnapshot {
            cols: self.size.cols,
            rows: self.size.rows,
            chars,
            fg,
            bg,
            flags,
            cursor_col: content.cursor.point.column.0.min(cols - 1) as u16,
            cursor_row: cursor_row.clamp(0, rows as i32 - 1) as u16,
            cursor,
            display_offset: display_offset as u32,
            history: self.term.grid().history_size() as u32,
            background: colors[NamedColor::Background]
                .map(rgb)
                .unwrap_or(self.palette.background),
            mouse_reporting: mode.intersects(TermMode::MOUSE_MODE),
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            bracketed_paste: mode.contains(TermMode::BRACKETED_PASTE),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emu() -> (Emulator, mpsc::UnboundedReceiver<TermSignal>) {
        Emulator::new(20, 5, 100, TerminalPalette::termoso_dark())
    }

    #[test]
    fn plain_text_lands_in_cells() {
        let (mut e, _rx) = emu();
        e.feed(b"hello\r\nworld");
        let s = e.snapshot();
        assert_eq!(s.cols, 20);
        assert_eq!(s.rows, 5);
        let row0: String = s.chars[..5]
            .iter()
            .map(|&c| char::from_u32(c).unwrap())
            .collect();
        assert_eq!(row0, "hello");
        assert_eq!(e.visible_text()[1], "world");
        assert_eq!((s.cursor_col, s.cursor_row), (5, 1));
        assert_eq!(s.cursor, CursorStyle::Block);
    }

    #[test]
    fn sgr_colours_and_attributes_resolve() {
        let (mut e, _rx) = emu();
        e.feed(b"\x1b[1;31mR\x1b[0m\x1b[7mI\x1b[0m\x1b[38;2;1;2;3mT");
        let s = e.snapshot();
        assert_eq!(s.flags[0] & flag::BOLD, flag::BOLD);
        assert_eq!(s.fg[0], TerminalPalette::termoso_dark().ansi[1]);
        // Inverse swaps fg/bg.
        assert_eq!(s.fg[1], TerminalPalette::termoso_dark().background);
        assert_eq!(s.bg[1], TerminalPalette::termoso_dark().foreground);
        assert_eq!(s.fg[2], 0x010203);
    }

    #[test]
    fn wide_chars_take_two_cells() {
        let (mut e, _rx) = emu();
        e.feed("日本".as_bytes());
        let s = e.snapshot();
        assert_eq!(s.chars[0], '日' as u32);
        assert_eq!(s.flags[0] & flag::WIDE, flag::WIDE);
        assert_eq!(s.chars[1], 0);
        assert_eq!(s.flags[1] & flag::WIDE_SPACER, flag::WIDE_SPACER);
        assert_eq!(s.chars[2], '本' as u32);
    }

    #[test]
    fn scrollback_and_display_offset() {
        let (mut e, _rx) = emu();
        for i in 0..12 {
            e.feed(format!("line{i}\r\n").as_bytes());
        }
        let s = e.snapshot();
        assert!(s.history >= 7);
        assert_eq!(s.display_offset, 0);
        e.scroll(3);
        let s = e.snapshot();
        assert_eq!(s.display_offset, 3);
        assert_eq!(e.visible_text()[0], "line5");
        e.scroll_to_bottom();
        assert_eq!(e.snapshot().display_offset, 0);
    }

    #[test]
    fn resize_keeps_content_and_reports_size() {
        let (mut e, _rx) = emu();
        e.feed(b"abc");
        e.resize(40, 10);
        let s = e.snapshot();
        assert_eq!((s.cols, s.rows), (40, 10));
        assert_eq!(s.chars.len(), 400);
        assert_eq!(e.visible_text()[0], "abc");
    }

    #[test]
    fn device_attributes_reply_goes_back_to_pty() {
        let (mut e, mut rx) = emu();
        e.feed(b"\x1b[c");
        match rx.try_recv() {
            Ok(TermSignal::PtyWrite(bytes)) => assert!(bytes.starts_with(b"\x1b[?")),
            other => panic!("expected DA reply, got {other:?}"),
        }
    }

    #[test]
    fn title_and_bell_signals() {
        let (mut e, mut rx) = emu();
        e.feed(b"\x1b]0;my title\x07\x07");
        match rx.try_recv() {
            Ok(TermSignal::Title(Some(t))) => assert_eq!(t, "my title"),
            other => panic!("expected title, got {other:?}"),
        }
        assert!(matches!(rx.try_recv(), Ok(TermSignal::Bell)));
    }

    #[test]
    fn modes_are_reported() {
        let (mut e, _rx) = emu();
        e.feed(b"\x1b[?1049h\x1b[?2004h\x1b[?1000h\x1b[?1h");
        let s = e.snapshot();
        assert!(s.alt_screen);
        assert!(s.bracketed_paste);
        assert!(s.mouse_reporting);
        assert!(s.app_cursor);
    }

    #[test]
    fn packed_frame_round_trips_cells() {
        let (mut e, _rx) = emu();
        e.feed(b"\x1b[1;31mR\x1b[0mx");
        let f = e.snapshot().pack();
        assert_eq!(f.cells.len(), 20 * 5 * CELL_BYTES);
        let word = |i: usize| u32::from_le_bytes(f.cells[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(word(0), 'R' as u32);
        assert_eq!(word(1) >> 24, flag::BOLD as u32);
        assert_eq!(word(1) & 0xFF_FFFF, TerminalPalette::termoso_dark().ansi[1]);
        assert_eq!(word(2), TerminalPalette::termoso_dark().background);
        assert_eq!(word(3), 'x' as u32);
        assert_eq!(word(4) >> 24, 0);
        assert_eq!((f.cursor_col, f.cursor_row), (2, 0));
    }

    #[test]
    fn indexed_palette_cube_and_greys() {
        let p = TerminalPalette::termoso_dark();
        assert_eq!(p.indexed(16), 0x000000);
        assert_eq!(p.indexed(231), 0xFFFFFF);
        assert_eq!(p.indexed(232), 0x080808);
        assert_eq!(p.indexed(255), 0xEEEEEE);
        assert_eq!(p.indexed(9), p.ansi[9]);
    }
}

#![no_main]
//! Bytes from a remote shell drive the mobile emulator; snapshot packing and
//! the input-mark bookkeeping run on top of the parser.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use termoso_mobile::{Emulator, TerminalPalette};

#[derive(Arbitrary, Debug)]
struct Input {
    cols: u8,
    rows: u8,
    chunks: Vec<Vec<u8>>,
    resize_to: Option<(u8, u8)>,
    scroll: i8,
}

fuzz_target!(|input: Input| {
    let (mut em, _rx) = Emulator::new(
        u16::from(input.cols).max(2),
        u16::from(input.rows).max(1),
        200,
        TerminalPalette::termoso_dark(),
    );
    let mark = em.input_mark();
    for chunk in &input.chunks {
        em.feed(chunk);
    }
    if let Some((c, r)) = input.resize_to {
        em.resize(u16::from(c).max(2), u16::from(r).max(1));
    }
    em.scroll(i32::from(input.scroll));
    let after = em.input_mark();
    let _ = em.input_between(&mark, &after);
    let _ = em.visible_text();
    let _ = em.snapshot().pack();
    em.scroll_to_bottom();
    let _ = em.mode();
});

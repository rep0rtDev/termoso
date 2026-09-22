#![no_main]
//! Private keys, certificates and public lines pasted or imported by the
//! user (OpenSSH, PEM, PuTTY .ppk).

use libfuzzer_sys::fuzz_target;
use termoso_core::hostkey::parse_public_key;
use termoso_core::keys::{
    certificate_matches, import, inspect, inspect_certificate, is_ppk, parse_public,
};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    let _ = is_ppk(s);
    let _ = import(s, None);
    let _ = import(s, Some("passphrase"));
    let _ = inspect(s);
    let _ = inspect_certificate(s);
    let _ = parse_public(s);
    let _ = parse_public_key(s);
    if let Some((a, b)) = s.split_once('\n') {
        let _ = certificate_matches(a, b);
    }
});

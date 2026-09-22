#![no_main]
//! Quick-connect / `ssh://` / `telnet://` strings typed or deep-linked into
//! the mobile app, plus the server URL normaliser.

use libfuzzer_sys::fuzz_target;
use termoso_mobile::{normalize_server_url, parse_target};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    if let Ok(t) = parse_target(s.to_string()) {
        assert!(!t.host.is_empty());
    }
    let _ = normalize_server_url(s);
});

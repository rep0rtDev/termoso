#![no_main]
//! IdP metadata is operator-supplied but fetched over the network at startup.

use libfuzzer_sys::fuzz_target;
use termoso_server::saml::metadata::parse_idp;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = parse_idp(s, None);
        let _ = parse_idp(s, Some("https://idp.example/metadata"));
    }
});

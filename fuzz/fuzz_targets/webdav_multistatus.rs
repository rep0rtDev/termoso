#![no_main]
//! PROPFIND 207 bodies come from whatever server the user pointed us at.

use libfuzzer_sys::fuzz_target;
use std::sync::LazyLock;
use termoso_core::webdav::parse_multistatus;
use url::Url;

static URLS: LazyLock<[(Url, &'static str); 2]> = LazyLock::new(|| {
    [
        (Url::parse("https://dav.example/remote.php/dav/files/u/").unwrap(), "/remote.php/dav/files/u"),
        (Url::parse("http://127.0.0.1:8080/").unwrap(), "/"),
    ]
});

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    for (url, base) in URLS.iter() {
        let _ = parse_multistatus(s, url, base);
    }
});

#![no_main]
//! Path / URL / fingerprint normalisation: no panics, and a normalised path
//! never escapes the root or keeps `.`/`..` segments.

use libfuzzer_sys::fuzz_target;
use termoso_core::webdav::{normalize_fingerprint, normalize_path, normalize_url};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    if let Ok(p) = normalize_path(s) {
        assert!(p.starts_with('/'), "{p:?}");
        assert!(!p.split('/').any(|seg| seg == "." || seg == ".."), "{p:?}");
        assert!(!p.contains("//"), "{p:?}");
        assert_eq!(normalize_path(&p).as_deref().ok(), Some(p.as_str()));
    }
    let _ = normalize_url(s);
    if let Ok(f) = normalize_fingerprint(s) {
        assert_eq!(normalize_fingerprint(&f).ok(), Some(f));
    }
});

#![no_main]
//! XML-DSig / XML-Enc / c14n primitives on arbitrary documents, the layer
//! below `consume_response` where structural confusion bugs would live.

use libfuzzer_sys::fuzz_target;
use std::sync::LazyLock;
use termoso_server::saml::c14n::{Method, canonicalize};
use termoso_server::saml::dsig::{cert_public_key, is_signed, verify_enveloped};
use termoso_server::saml::test_support::{idp_cert_der, sp_key};
use termoso_server::saml::xmlenc::decrypt_assertion;

static KEYS: LazyLock<Vec<rsa::RsaPublicKey>> =
    LazyLock::new(|| vec![cert_public_key(&idp_cert_der()).unwrap()]);
static SP_KEY: LazyLock<rsa::RsaPrivateKey> = LazyLock::new(sp_key);

const METHODS: [&str; 4] = [
    "http://www.w3.org/TR/2001/REC-xml-c14n-20010315",
    "http://www.w3.org/TR/2001/REC-xml-c14n-20010315#WithComments",
    "http://www.w3.org/2001/10/xml-exc-c14n#",
    "http://www.w3.org/2001/10/xml-exc-c14n#WithComments",
];

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };
    let Ok(doc) = roxmltree::Document::parse(s) else { return };
    let root = doc.root_element();
    for m in METHODS {
        let method = Method::from_algorithm(m, vec!["#default".into(), "saml".into()]).unwrap();
        let _ = canonicalize(root, None, &method);
        let _ = canonicalize(root, root.first_element_child(), &method);
    }
    for el in doc.descendants().filter(|n| n.is_element()) {
        if is_signed(el) {
            let _ = verify_enveloped(&doc, el, &KEYS, true);
            let _ = verify_enveloped(&doc, el, &KEYS, false);
        }
        if el.tag_name().name() == "EncryptedAssertion" {
            let _ = decrypt_assertion(el, &SP_KEY, true);
        }
    }
});

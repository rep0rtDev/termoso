//! Writes seed corpora under `corpus/<target>/` so the fuzzers start from
//! well-formed inputs. Run with `cargo +nightly run --example seed`.

use std::fs;
use std::path::Path;
use termoso_server::saml::test_support::{
    ResponseSpec, build_response_xml, idp_metadata_xml, idp_metadata_xml_with, sp_key,
};
use termoso_server::saml::metadata;

const IDP: &str = "https://idp.example/metadata";
const SP: &str = "https://sp.example/api/v1/auth/sso/corp/saml/metadata";
const ACS: &str = "https://sp.example/api/v1/auth/sso/saml/acs";

fn put(target: &str, name: &str, data: impl AsRef<[u8]>) {
    let dir = Path::new("corpus").join(target);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), data).unwrap();
}

fn main() {
    let base = || ResponseSpec::new(IDP, SP, ACS, "_req-fuzz", "alice@example.com");
    let mut variants: Vec<(&str, ResponseSpec)> = vec![("signed_assertion", base())];
    let mut s = base();
    s.sign_response = true;
    s.sign_assertion = false;
    variants.push(("signed_response", s));
    let mut s = base();
    s.sign_response = true;
    variants.push(("signed_both", s));
    let mut s = base();
    s.encrypt_for = Some(sp_key().public_key().clone());
    variants.push(("encrypted", s));
    let mut s = base();
    s.attributes = vec![
        ("email".into(), "bob@example.com".into()),
        ("displayName".into(), "Bob".into()),
    ];
    s.name_id_format = Some("urn:oasis:names:tc:SAML:2.0:nameid-format:persistent".into());
    variants.push(("attributes", s));
    let mut s = base();
    s.status = "urn:oasis:names:tc:SAML:2.0:status:Responder".into();
    variants.push(("failure_status", s));
    for (name, spec) in variants {
        let xml = build_response_xml(&spec);
        put("saml_response", name, &xml);
        put("saml_xml", name, &xml);
    }

    put("saml_metadata", "redirect", idp_metadata_xml(IDP, "https://idp.example/sso"));
    put(
        "saml_metadata",
        "post_signed",
        idp_metadata_xml_with(IDP, "https://idp.example/sso", metadata::BINDING_POST, true),
    );
    put("saml_xml", "metadata", idp_metadata_xml(IDP, "https://idp.example/sso"));

    put(
        "webdav_multistatus",
        "nextcloud",
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
 <d:response>
  <d:href>/remote.php/dav/files/u/Photos/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/></d:resourcetype>
    <d:getlastmodified>Sat, 19 Sep 2026 11:05:00 GMT</d:getlastmodified>
    <d:getetag>"abc"</d:getetag>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
  <d:propstat>
   <d:prop><d:getcontentlength/><d:getcontenttype/></d:prop>
   <d:status>HTTP/1.1 404 Not Found</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/files/u/Photos/sum%20mer.jpg</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype/>
    <d:getcontentlength>12345</d:getcontentlength>
    <d:getcontenttype>image/jpeg</d:getcontenttype>
    <d:getlastmodified>Fri, 18 Sep 2026 10:00:00 GMT</d:getlastmodified>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#,
    );
    put(
        "webdav_multistatus",
        "absolute_href",
        r#"<D:multistatus xmlns:D="DAV:"><D:response><D:href>http://127.0.0.1:8080/a%2Fb/../c</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>0</D:getcontentlength><D:creationdate>2026-09-19T11:05:00Z</D:creationdate></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>"#,
    );

    for (n, p) in ["/", "/a/./b/../c", "a//b/", "/%2e%2e/x", "/Фото/лето.jpg"].iter().enumerate() {
        put("webdav_path", &format!("p{n}"), p);
    }
    put("webdav_path", "url", "https://dav.example:8443/remote.php/dav/");
    put("webdav_path", "fp", "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=");

    put("terminal_feed", "osc52", b"\x1b]52;c;aGVsbG8=\x07");
    put("terminal_feed", "colors", b"\x1b[1;31mred\x1b[0m\r\n\x1b[38;2;10;20;30mrgb\x1b[m");
    put("terminal_feed", "osc133", b"\x1b]133;A\x07$ ls\r\n\x1b]133;C\x07out\r\n\x1b]133;D;0\x07");
    put("terminal_feed", "hyperlink", b"\x1b]8;;https://example.com\x07link\x1b]8;;\x07");
    put("terminal_feed", "csi", b"\x1b[2J\x1b[H\x1b[?1049h\x1b[10;20r\x1b[?25l\x1b[3;1Hx\x1b[?1049l");

    put(
        "ssh_keys",
        "ed25519",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW\nQyNTUxOQAAACDBv7SxOQeE48uY1fV1IhknmUVfX9xz4uDGqeoYFjhUmAAAAJhP4L7OT+C+\nzgAAAAtzc2gtZWQyNTUxOQAAACDBv7SxOQeE48uY1fV1IhknmUVfX9xz4uDGqeoYFjhUmA\nAAAEAuTs1Pf6/gG4y7X31kRmFfN3FhTBdk1q3xCPufx8z6cMG/tLE5B4Tjy5jV9XUiGSeZ\nRV9f3HPi4Map6hgWOFSYAAAAEHJlcG9ydEB0ZXJtb3NvLmNjAQIDBAU=\n-----END OPENSSH PRIVATE KEY-----\n",
    );
    put(
        "ssh_keys",
        "public",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMG/tLE5B4Tjy5jV9XUiGSeZRV9f3HPi4Map6hgWOFSY comment\n",
    );
    put(
        "ssh_keys",
        "ppk",
        "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\nComment: eddsa-key\nPublic-Lines: 2\nAAAAC3NzaC1lZDI1NTE5AAAAIMG/tLE5B4Tjy5jV9XUiGSeZRV9f3HPi4Map6hgW\nOFSY\nPrivate-Lines: 1\nAAAAIC5OzU9/r+AbjLtffWRGYV83cWFMF2TWrfEI+5/HzPpw\nPrivate-MAC: 0000000000000000000000000000000000000000000000000000000000000000\n",
    );

    for (n, t) in [
        "ssh://root@10.0.0.1:2222",
        "telnet://bbs.example:23",
        "user@host",
        "[::1]:22",
        "https://sync.example.com/",
    ]
    .iter()
    .enumerate()
    {
        put("quick_target", &format!("t{n}"), t);
    }
}

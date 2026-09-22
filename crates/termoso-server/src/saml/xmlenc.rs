//! XML Encryption for `saml:EncryptedAssertion`: RSA key transport
//! (`rsa-1_5`, `rsa-oaep-mgf1p`, `rsa-oaep`) wrapping an AES-CBC or AES-GCM
//! content key. Returns the decrypted `saml:Assertion` XML re-wrapped with
//! the namespaces that were in scope where the encrypted element sat, so the
//! fragment parses (and canonicalizes) exactly as the IdP signed it.

use aes::cipher::{BlockDecryptMut, KeyIvInit};
use aes_gcm::aead::Aead;
use aes_gcm::{Aes128Gcm, Aes256Gcm, KeyInit};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use roxmltree::Node;
use rsa::{Oaep, Pkcs1v15Encrypt, RsaPrivateKey};

pub const NS: &str = "http://www.w3.org/2001/04/xmlenc#";
const DS_NS: &str = super::dsig::NS;

const RSA_1_5: &str = "http://www.w3.org/2001/04/xmlenc#rsa-1_5";
const RSA_OAEP_MGF1P: &str = "http://www.w3.org/2001/04/xmlenc#rsa-oaep-mgf1p";
const RSA_OAEP: &str = "http://www.w3.org/2009/xmlenc11#rsa-oaep";
const AES128_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes128-cbc";
const AES192_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes192-cbc";
const AES256_CBC: &str = "http://www.w3.org/2001/04/xmlenc#aes256-cbc";
const AES128_GCM: &str = "http://www.w3.org/2009/xmlenc11#aes128-gcm";
const AES256_GCM: &str = "http://www.w3.org/2009/xmlenc11#aes256-gcm";
const MGF1_SHA1: &str = "http://www.w3.org/2009/xmlenc11#mgf1sha1";
const MGF1_SHA256: &str = "http://www.w3.org/2009/xmlenc11#mgf1sha256";

#[derive(Debug, thiserror::Error)]
pub enum EncError {
    #[error("malformed EncryptedAssertion: {0}")]
    Malformed(&'static str),
    #[error("unsupported encryption algorithm: {0}")]
    Unsupported(String),
    #[error("decryption failed")]
    Decrypt,
}

fn child<'a, 'i>(n: Node<'a, 'i>, ns: &str, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| {
        c.is_element() && c.tag_name().namespace() == Some(ns) && c.tag_name().name() == name
    })
}

fn cipher_value(n: Node) -> Result<Vec<u8>, EncError> {
    let v = child(n, NS, "CipherData")
        .and_then(|d| child(d, NS, "CipherValue"))
        .and_then(|v| v.text())
        .ok_or(EncError::Malformed("CipherValue"))?;
    STANDARD
        .decode(v.chars().filter(|c| !c.is_whitespace()).collect::<String>())
        .map_err(|_| EncError::Malformed("CipherValue base64"))
}

/// Decrypt `saml:EncryptedAssertion` → assertion XML text.
pub fn decrypt_assertion(encrypted: Node, sp_key: &RsaPrivateKey) -> Result<String, EncError> {
    let data = child(encrypted, NS, "EncryptedData").ok_or(EncError::Malformed("EncryptedData"))?;
    let data_alg = child(data, NS, "EncryptionMethod")
        .and_then(|m| m.attribute("Algorithm"))
        .ok_or(EncError::Malformed("EncryptionMethod"))?;

    // The EncryptedKey lives in ds:KeyInfo, or next to EncryptedData (possibly
    // referenced via ds:RetrievalMethod). Any of those placements is accepted.
    let key_info = child(data, DS_NS, "KeyInfo");
    let enc_key = key_info
        .and_then(|k| child(k, NS, "EncryptedKey"))
        .or_else(|| {
            let uri = key_info
                .and_then(|k| child(k, DS_NS, "RetrievalMethod"))
                .and_then(|r| r.attribute("URI"))
                .and_then(|u| u.strip_prefix('#'));
            encrypted.children().find(|c| {
                c.is_element()
                    && c.tag_name().namespace() == Some(NS)
                    && c.tag_name().name() == "EncryptedKey"
                    && uri.is_none_or(|u| c.attribute("Id") == Some(u))
            })
        })
        .ok_or(EncError::Malformed("EncryptedKey"))?;
    let key_method = child(enc_key, NS, "EncryptionMethod")
        .ok_or(EncError::Malformed("key EncryptionMethod"))?;
    let key_alg = key_method.attribute("Algorithm").unwrap_or("");
    let wrapped = cipher_value(enc_key)?;

    let content_key = match key_alg {
        RSA_1_5 => sp_key.decrypt(Pkcs1v15Encrypt, &wrapped),
        RSA_OAEP_MGF1P => sp_key.decrypt(Oaep::new::<sha1::Sha1>(), &wrapped),
        RSA_OAEP => {
            let digest = child(key_method, DS_NS, "DigestMethod")
                .and_then(|d| d.attribute("Algorithm"))
                .unwrap_or(super::dsig::DIGEST_SHA1);
            let mgf = child(key_method, "http://www.w3.org/2009/xmlenc11#", "MGF")
                .and_then(|d| d.attribute("Algorithm"))
                .unwrap_or(MGF1_SHA1);
            let padding = match (digest, mgf) {
                (super::dsig::DIGEST_SHA1, MGF1_SHA1) => Oaep::new::<sha1::Sha1>(),
                (super::dsig::DIGEST_SHA256, MGF1_SHA256) => Oaep::new::<sha2::Sha256>(),
                (super::dsig::DIGEST_SHA256, MGF1_SHA1) => {
                    Oaep::new_with_mgf_hash::<sha2::Sha256, sha1::Sha1>()
                }
                _ => return Err(EncError::Unsupported(format!("{key_alg} {digest} {mgf}"))),
            };
            sp_key.decrypt(padding, &wrapped)
        }
        other => return Err(EncError::Unsupported(other.into())),
    }
    .map_err(|_| EncError::Decrypt)?;

    let ciphertext = cipher_value(data)?;
    let plain = match data_alg {
        AES128_CBC => cbc_decrypt::<aes::Aes128>(&content_key, &ciphertext)?,
        AES192_CBC => cbc_decrypt::<aes::Aes192>(&content_key, &ciphertext)?,
        AES256_CBC => cbc_decrypt::<aes::Aes256>(&content_key, &ciphertext)?,
        AES128_GCM => gcm_decrypt::<Aes128Gcm>(&content_key, &ciphertext)?,
        AES256_GCM => gcm_decrypt::<Aes256Gcm>(&content_key, &ciphertext)?,
        other => return Err(EncError::Unsupported(other.into())),
    };
    let fragment = String::from_utf8(plain).map_err(|_| EncError::Decrypt)?;
    let fragment = fragment.trim_start_matches('\u{feff}').trim();

    // Re-establish the namespaces the fragment inherited from the response.
    let mut wrap = String::from("<w");
    for ns in encrypted.namespaces() {
        match ns.name() {
            Some(p) => {
                wrap.push_str(&format!(" xmlns:{p}=\"{}\"", esc(ns.uri())));
            }
            None => wrap.push_str(&format!(" xmlns=\"{}\"", esc(ns.uri()))),
        }
    }
    wrap.push('>');
    wrap.push_str(fragment);
    wrap.push_str("</w>");
    Ok(wrap)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn cbc_decrypt<C>(key: &[u8], data: &[u8]) -> Result<Vec<u8>, EncError>
where
    C: aes::cipher::BlockCipher + aes::cipher::BlockDecrypt + aes::cipher::KeyInit,
{
    let (iv, body) = data.split_at_checked(16).ok_or(EncError::Decrypt)?;
    if body.is_empty() || body.len() % 16 != 0 {
        return Err(EncError::Decrypt);
    }
    let dec = cbc::Decryptor::<C>::new_from_slices(key, iv).map_err(|_| EncError::Decrypt)?;
    let mut buf = body.to_vec();
    dec.decrypt_padded_mut::<aes::cipher::block_padding::NoPadding>(&mut buf)
        .map_err(|_| EncError::Decrypt)?;
    // XML Encryption padding: last byte = number of padding bytes (1..=16),
    // the others are arbitrary (not PKCS#7).
    let pad = *buf.last().ok_or(EncError::Decrypt)? as usize;
    if pad == 0 || pad > 16 || pad > buf.len() {
        return Err(EncError::Decrypt);
    }
    buf.truncate(buf.len() - pad);
    Ok(buf)
}

fn gcm_decrypt<A>(key: &[u8], data: &[u8]) -> Result<Vec<u8>, EncError>
where
    A: Aead + KeyInit,
{
    let (iv, body) = data.split_at_checked(12).ok_or(EncError::Decrypt)?;
    let cipher = A::new_from_slice(key).map_err(|_| EncError::Decrypt)?;
    cipher
        .decrypt(iv.into(), body)
        .map_err(|_| EncError::Decrypt)
}

/// Encrypt an assertion for `sp_cert_der` (AES-256-CBC + RSA-OAEP-MGF1P):
/// what a test IdP needs to exercise the decrypt path.
pub fn encrypt_assertion(assertion_xml: &str, sp_public: &rsa::RsaPublicKey) -> String {
    use aes::cipher::BlockEncryptMut;
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut key = [0u8; 32];
    rng.fill_bytes(&mut key);
    let mut iv = [0u8; 16];
    rng.fill_bytes(&mut iv);
    let mut plain = assertion_xml.as_bytes().to_vec();
    let pad = 16 - (plain.len() % 16);
    plain.extend(std::iter::repeat_n(0u8, pad - 1));
    plain.push(pad as u8);
    let enc = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into());
    let n = plain.len();
    let ct = enc
        .encrypt_padded_mut::<aes::cipher::block_padding::NoPadding>(&mut plain, n)
        .unwrap()
        .to_vec();
    let mut data = iv.to_vec();
    data.extend(ct);
    let wrapped = sp_public
        .encrypt(&mut rng, Oaep::new::<sha1::Sha1>(), &key)
        .unwrap();
    format!(
        concat!(
            r#"<saml:EncryptedAssertion xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion">"#,
            r#"<xenc:EncryptedData xmlns:xenc="http://www.w3.org/2001/04/xmlenc#" Type="http://www.w3.org/2001/04/xmlenc#Element">"#,
            r#"<xenc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#aes256-cbc"/>"#,
            r#"<ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><xenc:EncryptedKey>"#,
            r#"<xenc:EncryptionMethod Algorithm="http://www.w3.org/2001/04/xmlenc#rsa-oaep-mgf1p"/>"#,
            r#"<xenc:CipherData><xenc:CipherValue>{k}</xenc:CipherValue></xenc:CipherData>"#,
            r#"</xenc:EncryptedKey></ds:KeyInfo>"#,
            r#"<xenc:CipherData><xenc:CipherValue>{d}</xenc:CipherValue></xenc:CipherData>"#,
            r#"</xenc:EncryptedData></saml:EncryptedAssertion>"#
        ),
        k = STANDARD.encode(wrapped),
        d = STANDARD.encode(data)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
        let assertion = r#"<saml:Assertion ID="_a" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"><saml:Issuer>idp</saml:Issuer></saml:Assertion>"#;
        let enc = encrypt_assertion(assertion, &key.to_public_key());
        let doc = roxmltree::Document::parse(&enc).unwrap();
        let out = decrypt_assertion(doc.root_element(), &key).unwrap();
        assert!(out.contains(assertion));
        let wrong = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
        assert!(matches!(
            decrypt_assertion(doc.root_element(), &wrong),
            Err(EncError::Decrypt)
        ));
    }
}

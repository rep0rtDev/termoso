//! CTAP2 client over any transport.
//!
//! Implements what an SSH client needs from the *Client to Authenticator
//! Protocol* (FIDO Alliance, CTAP 2.1): `authenticatorGetInfo`,
//! `authenticatorClientPIN` (PIN/UV auth protocols 1 and 2),
//! `authenticatorMakeCredential`, `authenticatorGetAssertion` and
//! `authenticatorCredentialManagement` (resident keys). Framing is left to
//! a [`CtapTransport`]: CTAPHID in [`super::hid`], ISO 7816 APDUs in
//! [`super::nfc`].
//!
//! Only public material and the (short-lived) PIN/UV auth token ever live
//! here; the private scalar stays on the token and the PIN itself is hashed
//! and encrypted to the token's ephemeral key before it goes on the wire.

use std::collections::BTreeMap;

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use ciborium::Value;
use hmac::{Hmac, Mac};
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use russh::keys::ssh_key::PrivateKey;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::{
    AssertionData, CosePublicKey, Credential, FLAG_RESIDENT, FLAG_USER_PRESENCE, Fido2Device,
    Fido2Error, GenerateOptions, SkAlgorithm, SkHandle, signature_blob, sk_material,
};
use crate::error::Result;
use crate::keys::KeyMaterial;

/// `authenticatorMakeCredential`.
pub const CMD_MAKE_CREDENTIAL: u8 = 0x01;
/// `authenticatorGetAssertion`.
pub const CMD_GET_ASSERTION: u8 = 0x02;
/// `authenticatorGetInfo`.
pub const CMD_GET_INFO: u8 = 0x04;
/// `authenticatorClientPIN`.
pub const CMD_CLIENT_PIN: u8 = 0x06;
/// `authenticatorCredentialManagement` (CTAP 2.1).
pub const CMD_CRED_MGMT: u8 = 0x0A;
/// `authenticatorCredentialManagement` as shipped before 2.1 (`credentialMgmtPreview`).
pub const CMD_CRED_MGMT_PREVIEW: u8 = 0x41;

const PIN_GET_RETRIES: u8 = 0x01;
const PIN_GET_KEY_AGREEMENT: u8 = 0x02;
const PIN_GET_TOKEN: u8 = 0x05;
const PIN_GET_TOKEN_WITH_PERMISSIONS: u8 = 0x09;

const CM_ENUMERATE_RPS_BEGIN: u8 = 0x02;
const CM_ENUMERATE_RPS_NEXT: u8 = 0x03;
const CM_ENUMERATE_CREDS_BEGIN: u8 = 0x04;
const CM_ENUMERATE_CREDS_NEXT: u8 = 0x05;

/// PIN/UV auth token permission bits (CTAP 2.1 §6.5.5.7).
const PERM_MAKE_CREDENTIAL: u8 = 0x01;
const PERM_GET_ASSERTION: u8 = 0x02;
const PERM_CREDENTIAL_MANAGEMENT: u8 = 0x04;

/// Moves one CTAP2 message to the authenticator and brings the response
/// back. `payload` is `command byte ‖ CBOR parameters`; the result is
/// `status byte ‖ CBOR response`. Blocking, including the wait for a touch –
/// keep-alives are the transport's business.
pub trait CtapTransport: Send {
    /// One request/response exchange.
    fn cbor(&mut self, payload: &[u8]) -> std::result::Result<Vec<u8>, Fido2Error>;
}

/// `authenticatorGetInfo`, the parts we look at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    /// `FIDO_2_0`, `FIDO_2_1_PRE`, `FIDO_2_1`, `U2F_V2`…
    pub versions: Vec<String>,
    /// Supported extensions.
    pub extensions: Vec<String>,
    /// Authenticator attestation GUID.
    pub aaguid: Vec<u8>,
    /// Option map (`rk`, `up`, `uv`, `clientPin`, `credMgmt`, `pinUvAuthToken`…).
    pub options: BTreeMap<String, bool>,
    /// Largest message the token accepts (bytes), when advertised.
    pub max_msg_size: Option<u64>,
    /// PIN/UV auth protocols, in the token's order of preference.
    pub pin_protocols: Vec<u64>,
    /// COSE algorithm identifiers the token can mint (`-7`, `-8`…).
    pub algorithms: Vec<i64>,
}

impl Info {
    fn option(&self, name: &str) -> Option<bool> {
        self.options.get(name).copied()
    }

    /// Whether the option is present *and* true.
    pub fn has(&self, name: &str) -> bool {
        self.option(name) == Some(true)
    }

    /// Whether a client PIN is currently set (`None`: PINs unsupported).
    pub fn pin_set(&self) -> Option<bool> {
        self.option("clientPin")
    }

    /// Credential-management command the token speaks, if any.
    pub fn cred_mgmt_cmd(&self) -> Option<u8> {
        if self.has("credMgmt") {
            Some(CMD_CRED_MGMT)
        } else if self.has("credentialMgmtPreview") {
            Some(CMD_CRED_MGMT_PREVIEW)
        } else {
            None
        }
    }

    /// Key types the token can generate. ECDSA P-256 is mandatory for every
    /// FIDO2 token; tokens that predate CTAP 2.1 do not advertise the list.
    pub fn sk_algorithms(&self) -> Vec<SkAlgorithm> {
        let mut out = vec![SkAlgorithm::EcdsaP256];
        if self.algorithms.contains(&-8) {
            out.push(SkAlgorithm::Ed25519);
        }
        out
    }

    /// Present the token to the UI. Transport facts (`path`, product name,
    /// USB ids) come from the caller; the rest is `getInfo`.
    pub fn describe(
        &self,
        path: &str,
        product: &str,
        vendor_id: u16,
        product_id: u16,
    ) -> Fido2Device {
        Fido2Device {
            path: path.to_string(),
            product: product.to_string(),
            vendor_id,
            product_id,
            aaguid: (!self.aaguid.is_empty()).then(|| hex::encode(&self.aaguid)),
            pin_set: self.pin_set(),
            resident_keys: self.has("rk"),
            algorithms: self.sk_algorithms(),
            versions: self.versions.clone(),
        }
    }
}

/// A resident credential the token enumerated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidentCredential {
    /// Relying party id (`ssh:`…).
    pub application: String,
    /// The credential (handle + public key).
    pub credential: Credential,
    /// User name recorded at creation.
    pub user_name: String,
}

// ---------------------------------------------------------------------------
// CBOR helpers

pub(super) fn int(k: i64) -> Value {
    Value::Integer(k.into())
}

pub(super) fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

pub(super) fn bytes(b: &[u8]) -> Value {
    Value::Bytes(b.to_vec())
}

/// Map with keys in CTAP canonical order (shorter encodings first, then
/// bytewise) – some tokens reject anything else.
pub(super) fn map(entries: Vec<(Value, Value)>) -> Value {
    let mut entries: Vec<(Vec<u8>, Value, Value)> = entries
        .into_iter()
        .map(|(k, v)| (encode(&k), k, v))
        .collect();
    entries.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
    Value::Map(entries.into_iter().map(|(_, k, v)| (k, v)).collect())
}

pub(super) fn encode(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    ciborium::into_writer(v, &mut out).expect("CBOR encoding into a Vec cannot fail");
    out
}

pub(super) fn decode(b: &[u8]) -> std::result::Result<Value, Fido2Error> {
    ciborium::from_reader(b).map_err(|e| Fido2Error::Other(format!("bad CBOR from token: {e}")))
}

/// Decode one CBOR item and return it with the bytes that follow it.
fn decode_prefix(b: &[u8]) -> std::result::Result<(Value, &[u8]), Fido2Error> {
    let mut cursor = std::io::Cursor::new(b);
    let v: Value = ciborium::from_reader(&mut cursor)
        .map_err(|e| Fido2Error::Other(format!("bad CBOR from token: {e}")))?;
    let used = cursor.position() as usize;
    Ok((v, &b[used..]))
}

/// Field access on CTAP response maps (integer keys).
pub(super) trait Field {
    fn field(&self, key: i64) -> Option<&Value>;
    fn field_text(&self, key: &str) -> Option<&Value>;
}

impl Field for Value {
    fn field(&self, key: i64) -> Option<&Value> {
        self.as_map()?
            .iter()
            .find(|(k, _)| k.as_integer().and_then(|i| i128::from(i).try_into().ok()) == Some(key))
            .map(|(_, v)| v)
    }

    fn field_text(&self, key: &str) -> Option<&Value> {
        self.as_map()?
            .iter()
            .find(|(k, _)| k.as_text() == Some(key))
            .map(|(_, v)| v)
    }
}

pub(super) fn as_bytes(v: Option<&Value>, what: &str) -> std::result::Result<Vec<u8>, Fido2Error> {
    v.and_then(Value::as_bytes)
        .cloned()
        .ok_or_else(|| Fido2Error::Other(format!("token response lacks {what}")))
}

pub(super) fn as_u64(v: &Value) -> Option<u64> {
    v.as_integer().and_then(|i| u64::try_from(i).ok())
}

pub(super) fn as_i64(v: &Value) -> Option<i64> {
    v.as_integer().and_then(|i| i64::try_from(i).ok())
}

pub(super) fn cose_public_key(v: &Value) -> std::result::Result<CosePublicKey, Fido2Error> {
    let bad = |m: &str| Fido2Error::Other(format!("bad COSE key from token: {m}"));
    let kty = v.field(1).and_then(as_i64).ok_or_else(|| bad("kty"))?;
    let crv = v.field(-1).and_then(as_i64).ok_or_else(|| bad("crv"))?;
    let x: [u8; 32] = as_bytes(v.field(-2), "x")?
        .try_into()
        .map_err(|_| bad("x length"))?;
    match (kty, crv) {
        (1, 6) => Ok(CosePublicKey::Ed25519(x)),
        (2, 1) => {
            let y: [u8; 32] = as_bytes(v.field(-3), "y")?
                .try_into()
                .map_err(|_| bad("y length"))?;
            Ok(CosePublicKey::P256 { x, y })
        }
        _ => Err(Fido2Error::Unsupported(format!(
            "COSE key type {kty}/curve {crv}"
        ))),
    }
}

/// `attestedCredentialData` out of `authData` (after `rpIdHash ‖ flags ‖ counter`).
pub(super) fn attested_credential(auth_data: &[u8]) -> std::result::Result<Credential, Fido2Error> {
    let short = || Fido2Error::Other("short authenticator data".into());
    let flags = *auth_data.get(32).ok_or_else(short)?;
    if flags & 0x40 == 0 {
        return Err(Fido2Error::Other(
            "token returned no attested credential data".into(),
        ));
    }
    let rest = auth_data.get(37..).ok_or_else(short)?;
    let rest = rest.get(16..).ok_or_else(short)?; // aaguid
    let len = u16::from_be_bytes([
        *rest.first().ok_or_else(short)?,
        *rest.get(1).ok_or_else(short)?,
    ]) as usize;
    let id = rest.get(2..2 + len).ok_or_else(short)?.to_vec();
    let (key, _extensions) = decode_prefix(rest.get(2 + len..).ok_or_else(short)?)?;
    Ok(Credential {
        id,
        public_key: cose_public_key(&key)?,
    })
}

// ---------------------------------------------------------------------------
// PIN/UV auth protocols

type HmacSha256 = Hmac<Sha256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Shared secret with the token for one PIN exchange (protocol 1: 32 bytes
/// used for both AES and HMAC; protocol 2: `hmacKey ‖ aesKey`).
pub(super) struct SharedSecret {
    protocol: u64,
    key: Zeroizing<Vec<u8>>,
}

impl SharedSecret {
    pub(super) fn new(protocol: u64, z: &[u8]) -> Self {
        let key = match protocol {
            1 => Sha256::digest(z).to_vec(),
            _ => {
                let hk = hkdf::Hkdf::<Sha256>::new(Some(&[0u8; 32]), z);
                let mut hmac_key = [0u8; 32];
                let mut aes_key = [0u8; 32];
                hk.expand(b"CTAP2 HMAC key", &mut hmac_key)
                    .expect("32 bytes is a valid HKDF length");
                hk.expand(b"CTAP2 AES key", &mut aes_key)
                    .expect("32 bytes is a valid HKDF length");
                let mut k = hmac_key.to_vec();
                k.extend_from_slice(&aes_key);
                k
            }
        };
        Self {
            protocol,
            key: Zeroizing::new(key),
        }
    }

    #[cfg(test)]
    fn hmac_key(&self) -> &[u8] {
        &self.key[..32]
    }

    fn aes_key(&self) -> &[u8] {
        match self.protocol {
            1 => &self.key[..32],
            _ => &self.key[32..64],
        }
    }

    pub(super) fn encrypt(&self, plain: &[u8]) -> Vec<u8> {
        let iv: [u8; 16] = match self.protocol {
            1 => [0u8; 16],
            _ => {
                let mut iv = [0u8; 16];
                rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut iv);
                iv
            }
        };
        let mut out = cbc_encrypt(&iv, plain, self.aes_key());
        if self.protocol != 1 {
            let mut with_iv = iv.to_vec();
            with_iv.append(&mut out);
            out = with_iv;
        }
        out
    }

    pub(super) fn decrypt(&self, ct: &[u8]) -> std::result::Result<Zeroizing<Vec<u8>>, Fido2Error> {
        let (iv, body): ([u8; 16], &[u8]) = match self.protocol {
            1 => ([0u8; 16], ct),
            _ => {
                let iv = ct
                    .get(..16)
                    .ok_or_else(|| Fido2Error::Other("short ciphertext from token".into()))?;
                (iv.try_into().expect("16 bytes"), &ct[16..])
            }
        };
        if body.is_empty() || body.len() % 16 != 0 {
            return Err(Fido2Error::Other("bad ciphertext length from token".into()));
        }
        let mut buf = Zeroizing::new(body.to_vec());
        let mut dec =
            Aes256CbcDec::new_from_slices(self.aes_key(), &iv).expect("32-byte key, 16-byte IV");
        for block in buf.as_chunks_mut::<16>().0 {
            dec.decrypt_block_mut(aes::Block::from_mut_slice(block));
        }
        Ok(buf)
    }

    #[cfg(test)]
    fn authenticate(&self, msg: &[u8]) -> Vec<u8> {
        authenticate(self.protocol, self.hmac_key(), msg)
    }
}

/// AES-256-CBC without padding (CTAP messages are block-aligned).
fn cbc_encrypt(iv: &[u8; 16], plain: &[u8], key: &[u8]) -> Vec<u8> {
    debug_assert_eq!(plain.len() % 16, 0);
    let mut buf = plain.to_vec();
    let mut enc = Aes256CbcEnc::new_from_slices(key, iv).expect("32-byte key, 16-byte IV");
    for block in buf.as_chunks_mut::<16>().0 {
        enc.encrypt_block_mut(aes::Block::from_mut_slice(block));
    }
    buf
}

/// `authenticate(key, message)`: protocol 1 truncates the HMAC to 16 bytes,
/// protocol 2 sends all 32.
pub(super) fn authenticate(protocol: u64, key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(msg);
    let tag = mac.finalize().into_bytes();
    match protocol {
        1 => tag[..16].to_vec(),
        _ => tag.to_vec(),
    }
}

/// PIN/UV auth token for a session, bound to the protocol it came through.
pub struct PinToken {
    protocol: u64,
    token: Zeroizing<Vec<u8>>,
}

impl PinToken {
    /// `pinUvAuthParam` over `msg`.
    pub fn authenticate(&self, msg: &[u8]) -> Vec<u8> {
        authenticate(self.protocol, &self.token, msg)
    }

    /// Protocol number to put next to the param.
    pub fn protocol(&self) -> u64 {
        self.protocol
    }
}

// ---------------------------------------------------------------------------
// Client

/// A CTAP2 authenticator reached through `T`.
pub struct Authenticator<T: CtapTransport> {
    transport: T,
    info: Info,
}

impl<T: CtapTransport> Authenticator<T> {
    /// Connect and read `getInfo`.
    pub fn open(transport: T) -> std::result::Result<Self, Fido2Error> {
        let mut auth = Self {
            transport,
            info: Info::default(),
        };
        auth.info = auth.get_info()?;
        Ok(auth)
    }

    /// Cached `getInfo`.
    pub fn info(&self) -> &Info {
        &self.info
    }

    /// Give the transport back.
    pub fn into_transport(self) -> T {
        self.transport
    }

    /// One command; `Ok(None)` when the token answered with a bare status.
    fn call(
        &mut self,
        cmd: u8,
        params: Option<Value>,
    ) -> std::result::Result<Option<Value>, Fido2Error> {
        let mut payload = vec![cmd];
        if let Some(p) = params {
            payload.extend(encode(&p));
        }
        let response = self.transport.cbor(&payload)?;
        let (&status, body) = response
            .split_first()
            .ok_or_else(|| Fido2Error::Other("empty response from token".into()))?;
        if status != 0 {
            return Err(Fido2Error::from_status(status));
        }
        if body.is_empty() {
            return Ok(None);
        }
        Ok(Some(decode(body)?))
    }

    fn get_info(&mut self) -> std::result::Result<Info, Fido2Error> {
        let v = self
            .call(CMD_GET_INFO, None)?
            .ok_or_else(|| Fido2Error::Other("empty getInfo".into()))?;
        let strings = |k: i64| -> Vec<String> {
            v.field(k)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_text().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let mut options = BTreeMap::new();
        if let Some(m) = v.field(4).and_then(Value::as_map) {
            for (k, val) in m {
                if let (Some(k), Some(b)) = (k.as_text(), val.as_bool()) {
                    options.insert(k.to_string(), b);
                }
            }
        }
        let algorithms = v
            .field(10)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.field_text("alg").and_then(as_i64))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Info {
            versions: strings(1),
            extensions: strings(2),
            aaguid: v
                .field(3)
                .and_then(Value::as_bytes)
                .cloned()
                .unwrap_or_default(),
            options,
            max_msg_size: v.field(5).and_then(as_u64),
            pin_protocols: v
                .field(6)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(as_u64).collect())
                .unwrap_or_default(),
            algorithms,
        })
    }

    fn pin_protocol(&self) -> u64 {
        // Prefer 2 when offered; tokens list their preference first.
        if self.info.pin_protocols.contains(&2) {
            2
        } else {
            1
        }
    }

    /// ECDH with the token's ephemeral key agreement key.
    fn key_agreement(
        &mut self,
        protocol: u64,
    ) -> std::result::Result<(SharedSecret, Value), Fido2Error> {
        let v = self
            .call(
                CMD_CLIENT_PIN,
                Some(map(vec![
                    (int(1), int(protocol as i64)),
                    (int(2), int(PIN_GET_KEY_AGREEMENT as i64)),
                ])),
            )?
            .ok_or_else(|| Fido2Error::Other("empty keyAgreement".into()))?;
        let peer = v
            .field(1)
            .ok_or_else(|| Fido2Error::Other("token sent no key agreement key".into()))?;
        let CosePublicKey::P256 { x, y } = cose_public_key(peer)? else {
            return Err(Fido2Error::Other("key agreement key is not P-256".into()));
        };
        let mut sec1 = vec![0x04];
        sec1.extend_from_slice(&x);
        sec1.extend_from_slice(&y);
        let peer_key = p256::PublicKey::from_sec1_bytes(&sec1)
            .map_err(|_| Fido2Error::Other("bad key agreement point".into()))?;
        let ours = EphemeralSecret::random(&mut rand_core::OsRng);
        let our_point = ours.public_key().to_encoded_point(false);
        let z = ours.diffie_hellman(&peer_key);
        let secret = SharedSecret::new(protocol, z.raw_secret_bytes());
        let our_cose = map(vec![
            (int(1), int(2)),
            (int(3), int(-25)),
            (int(-1), int(1)),
            (int(-2), bytes(our_point.x().expect("uncompressed point"))),
            (int(-3), bytes(our_point.y().expect("uncompressed point"))),
        ]);
        Ok((secret, our_cose))
    }

    /// Remaining PIN attempts, when the token tells.
    pub fn pin_retries(&mut self) -> Option<i32> {
        let protocol = self.pin_protocol();
        let v = self
            .call(
                CMD_CLIENT_PIN,
                Some(map(vec![
                    (int(1), int(protocol as i64)),
                    (int(2), int(PIN_GET_RETRIES as i64)),
                ])),
            )
            .ok()??;
        v.field(3)
            .and_then(as_i64)
            .and_then(|n| i32::try_from(n).ok())
    }

    /// Exchange the PIN for a PIN/UV auth token. With `permissions` (CTAP
    /// 2.1 tokens) the token is scoped to those operations and `rp_id`.
    pub fn pin_token(
        &mut self,
        pin: &str,
        permissions: u8,
        rp_id: Option<&str>,
    ) -> std::result::Result<PinToken, Fido2Error> {
        if self.info.pin_set() == Some(false) {
            return Err(Fido2Error::PinNotSet);
        }
        let protocol = self.pin_protocol();
        let (secret, our_key) = self.key_agreement(protocol)?;
        let pin_hash = Sha256::digest(pin.as_bytes());
        let pin_hash_enc = secret.encrypt(&pin_hash[..16]);
        let mut params = vec![
            (int(1), int(protocol as i64)),
            (int(3), our_key),
            (int(6), bytes(&pin_hash_enc)),
        ];
        if self.info.has("pinUvAuthToken") {
            params.push((int(2), int(PIN_GET_TOKEN_WITH_PERMISSIONS as i64)));
            params.push((int(9), int(permissions as i64)));
            if let Some(rp) = rp_id {
                params.push((int(10), text(rp)));
            }
        } else {
            params.push((int(2), int(PIN_GET_TOKEN as i64)));
        }
        let v = match self.call(CMD_CLIENT_PIN, Some(map(params))) {
            Ok(v) => v.ok_or_else(|| Fido2Error::Other("empty pin token response".into()))?,
            Err(Fido2Error::PinInvalid { .. }) => {
                return Err(Fido2Error::PinInvalid {
                    retries: self.pin_retries(),
                });
            }
            Err(e) => return Err(e),
        };
        let enc = as_bytes(v.field(2), "pinUvAuthToken")?;
        let token = secret.decrypt(&enc)?;
        Ok(PinToken { protocol, token })
    }

    /// Create a credential. `pin` is exchanged for an auth token when
    /// given; tokens with a PIN set demand one for makeCredential.
    pub fn make_credential(
        &mut self,
        rp_id: &str,
        user_id: &[u8],
        user_name: &str,
        algorithm: SkAlgorithm,
        resident: bool,
        pin: Option<&str>,
    ) -> std::result::Result<Credential, Fido2Error> {
        if algorithm == SkAlgorithm::Ed25519
            && !self.info.algorithms.is_empty()
            && !self.info.algorithms.contains(&-8)
        {
            return Err(Fido2Error::Unsupported("Ed25519 keys".into()));
        }
        let client_data_hash: [u8; 32] = {
            let mut h = [0u8; 32];
            rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut h);
            h
        };
        let token = match pin {
            Some(p) => Some(self.pin_token(p, PERM_MAKE_CREDENTIAL, Some(rp_id))?),
            None => None,
        };
        let mut params = vec![
            (int(1), bytes(&client_data_hash)),
            (int(2), map(vec![(text("id"), text(rp_id))])),
            (
                int(3),
                map(vec![
                    (text("id"), bytes(user_id)),
                    (text("name"), text(user_name)),
                    (text("displayName"), text(user_name)),
                ]),
            ),
            (
                int(4),
                Value::Array(vec![map(vec![
                    (text("alg"), int(algorithm.cose_alg())),
                    (text("type"), text("public-key")),
                ])]),
            ),
        ];
        if resident {
            params.push((int(7), map(vec![(text("rk"), Value::Bool(true))])));
        }
        if let Some(t) = &token {
            params.push((int(8), bytes(&t.authenticate(&client_data_hash))));
            params.push((int(9), int(t.protocol() as i64)));
        }
        let v = self
            .call(CMD_MAKE_CREDENTIAL, Some(map(params)))?
            .ok_or_else(|| Fido2Error::Other("empty makeCredential response".into()))?;
        let auth_data = as_bytes(v.field(2), "authData")?;
        attested_credential(&auth_data)
    }

    /// Ask for an assertion over `client_data_hash` with the credential
    /// `credential_id` under `rp_id`.
    pub fn get_assertion(
        &mut self,
        rp_id: &str,
        client_data_hash: &[u8; 32],
        credential_id: &[u8],
        user_presence: bool,
        pin: Option<&str>,
    ) -> std::result::Result<AssertionData, Fido2Error> {
        let token = match pin {
            Some(p) => Some(self.pin_token(p, PERM_GET_ASSERTION, Some(rp_id))?),
            None => None,
        };
        let mut params = vec![
            (int(1), text(rp_id)),
            (int(2), bytes(client_data_hash)),
            (
                int(3),
                Value::Array(vec![map(vec![
                    (text("id"), bytes(credential_id)),
                    (text("type"), text("public-key")),
                ])]),
            ),
        ];
        if !user_presence {
            params.push((int(5), map(vec![(text("up"), Value::Bool(false))])));
        }
        if let Some(t) = &token {
            params.push((int(6), bytes(&t.authenticate(client_data_hash))));
            params.push((int(7), int(t.protocol() as i64)));
        }
        let v = match self.call(CMD_GET_ASSERTION, Some(map(params.clone()))) {
            // CTAP 2.0 tokens may refuse `up: false`; ask again the normal way.
            Err(Fido2Error::Unsupported(_)) if !user_presence => {
                params.retain(|(k, _)| k != &int(5));
                self.call(CMD_GET_ASSERTION, Some(map(params)))?
            }
            r => r?,
        }
        .ok_or_else(|| Fido2Error::Other("empty getAssertion response".into()))?;
        Ok(AssertionData {
            auth_data: as_bytes(v.field(2), "authData")?,
            signature: as_bytes(v.field(3), "signature")?,
        })
    }

    /// Resident credentials under relying parties starting with `ssh:`.
    pub fn resident_ssh_credentials(
        &mut self,
        pin: &str,
    ) -> std::result::Result<Vec<ResidentCredential>, Fido2Error> {
        let cmd = self
            .info
            .cred_mgmt_cmd()
            .ok_or_else(|| Fido2Error::Unsupported("credential management".into()))?;
        let token = self.pin_token(pin, PERM_CREDENTIAL_MANAGEMENT, None)?;

        let rps = match self.cred_mgmt(cmd, &token, CM_ENUMERATE_RPS_BEGIN, None) {
            Ok(v) => v,
            // No resident credentials at all.
            Err(Fido2Error::WrongDevice) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let total = rps.field(5).and_then(as_u64).unwrap_or(0);
        let mut rp_hashes = Vec::new();
        let mut push_rp = |v: &Value| -> std::result::Result<(), Fido2Error> {
            let id = v
                .field(3)
                .and_then(|rp| rp.field_text("id"))
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_string();
            let hash = as_bytes(v.field(4), "rpIDHash")?;
            rp_hashes.push((id, hash));
            Ok(())
        };
        push_rp(&rps)?;
        for _ in 1..total {
            let next = self.cred_mgmt_next(cmd, CM_ENUMERATE_RPS_NEXT)?;
            push_rp(&next)?;
        }

        let mut out = Vec::new();
        for (application, hash) in rp_hashes {
            if !application.starts_with("ssh:") {
                continue;
            }
            let params = map(vec![(int(1), bytes(&hash))]);
            let first = match self.cred_mgmt(cmd, &token, CM_ENUMERATE_CREDS_BEGIN, Some(params)) {
                Ok(v) => v,
                Err(Fido2Error::WrongDevice) => continue,
                Err(e) => return Err(e),
            };
            let total = first.field(9).and_then(as_u64).unwrap_or(1);
            let mut push_cred = |v: &Value| -> std::result::Result<(), Fido2Error> {
                let id = v
                    .field(7)
                    .and_then(|c| c.field_text("id"))
                    .and_then(Value::as_bytes)
                    .cloned()
                    .ok_or_else(|| Fido2Error::Other("credential without id".into()))?;
                let key = v
                    .field(8)
                    .ok_or_else(|| Fido2Error::Other("credential without public key".into()))?;
                let public_key = match cose_public_key(key) {
                    Ok(k) => k,
                    Err(Fido2Error::Unsupported(_)) => return Ok(()),
                    Err(e) => return Err(e),
                };
                let user_name = v
                    .field(6)
                    .and_then(|u| u.field_text("name"))
                    .and_then(Value::as_text)
                    .unwrap_or_default()
                    .to_string();
                out.push(ResidentCredential {
                    application: application.clone(),
                    credential: Credential { id, public_key },
                    user_name,
                });
                Ok(())
            };
            push_cred(&first)?;
            for _ in 1..total {
                let next = self.cred_mgmt_next(cmd, CM_ENUMERATE_CREDS_NEXT)?;
                push_cred(&next)?;
            }
        }
        Ok(out)
    }

    fn cred_mgmt(
        &mut self,
        cmd: u8,
        token: &PinToken,
        sub: u8,
        sub_params: Option<Value>,
    ) -> std::result::Result<Value, Fido2Error> {
        // pinUvAuthParam = authenticate(token, subCommand ‖ subCommandParams)
        let mut msg = vec![sub];
        if let Some(p) = &sub_params {
            msg.extend(encode(p));
        }
        let mut params = vec![
            (int(1), int(sub as i64)),
            (int(3), int(token.protocol() as i64)),
            (int(4), bytes(&token.authenticate(&msg))),
        ];
        if let Some(p) = sub_params {
            params.push((int(2), p));
        }
        self.call(cmd, Some(map(params)))?
            .ok_or_else(|| Fido2Error::Other("empty credential management response".into()))
    }

    fn cred_mgmt_next(&mut self, cmd: u8, sub: u8) -> std::result::Result<Value, Fido2Error> {
        self.call(cmd, Some(map(vec![(int(1), int(sub as i64))])))?
            .ok_or_else(|| Fido2Error::Other("empty credential management response".into()))
    }
}

// ---------------------------------------------------------------------------
// SSH-level operations

/// Create a credential on the token and wrap it as an OpenSSH `sk-*` key.
/// Blocking: waits for the touch.
pub fn generate_with<T: CtapTransport>(
    auth: &mut Authenticator<T>,
    opts: &GenerateOptions,
) -> Result<KeyMaterial> {
    let application = opts.application()?;
    if opts.resident && !auth.info().has("rk") {
        return Err(Fido2Error::Unsupported("resident keys".into()).into());
    }
    let cred = auth.make_credential(
        application,
        &opts.user_id(),
        opts.user_name(),
        opts.algorithm,
        opts.resident,
        opts.pin(),
    )?;
    sk_material(
        opts.algorithm,
        &cred,
        application,
        opts.flags(),
        &opts.comment,
        opts.passphrase.as_deref().map(|p| p.as_str()),
    )
}

/// Sign SSH `data` with the token holding `key`'s credential. Returns the
/// SSH signature blob (`string alg, string sig, byte flags, uint32 counter`).
/// Blocking: waits for the touch.
pub fn sign_with<T: CtapTransport>(
    auth: &mut Authenticator<T>,
    key: &PrivateKey,
    data: &[u8],
    pin: Option<&str>,
) -> Result<Vec<u8>> {
    let handle = SkHandle::of(key)?;
    let pin = pin.filter(|p| !p.is_empty());
    if handle.wants_uv() && pin.is_none() {
        return Err(Fido2Error::PinRequired.into());
    }
    // OpenSSH signs `message = SHA256(data)` as clientDataHash.
    let client_data_hash: [u8; 32] = Sha256::digest(data).into();
    let assertion = auth.get_assertion(
        &handle.application,
        &client_data_hash,
        &handle.key_handle,
        handle.wants_up(),
        pin,
    )?;
    signature_blob(handle.algorithm, &assertion)
}

/// Resident SSH credentials on the token, wrapped as OpenSSH keys
/// (`ssh-keygen -K`). Needs the PIN. Blocking.
pub fn load_resident_with<T: CtapTransport>(
    auth: &mut Authenticator<T>,
    pin: &str,
    passphrase: Option<&str>,
) -> Result<Vec<KeyMaterial>> {
    let creds = auth.resident_ssh_credentials(pin)?;
    creds
        .iter()
        .map(|c| {
            sk_material(
                c.credential.public_key.algorithm(),
                &c.credential,
                &c.application,
                FLAG_USER_PRESENCE | FLAG_RESIDENT,
                &c.user_name,
                passphrase,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_map_order() {
        let m = map(vec![
            (text("type"), text("public-key")),
            (int(-2), int(1)),
            (int(3), int(2)),
            (text("id"), int(0)),
            (int(1), int(9)),
        ]);
        let keys: Vec<Value> = m.into_map().unwrap().into_iter().map(|(k, _)| k).collect();
        // 1-byte ints first (1, 3, then -2 = 0x21), then 2-byte "id", then "type".
        assert_eq!(
            keys,
            vec![int(1), int(3), int(-2), text("id"), text("type")]
        );
    }

    #[test]
    fn pin_protocol_primitives_match_spec_shape() {
        let s1 = SharedSecret::new(1, &[7u8; 32]);
        let ct = s1.encrypt(&[1u8; 16]);
        assert_eq!(ct.len(), 16);
        assert_eq!(&*s1.decrypt(&ct).unwrap(), &[1u8; 16]);
        assert_eq!(s1.authenticate(b"x").len(), 16);

        let s2 = SharedSecret::new(2, &[7u8; 32]);
        let ct = s2.encrypt(&[2u8; 32]);
        assert_eq!(ct.len(), 16 + 32);
        assert_eq!(&*s2.decrypt(&ct).unwrap(), &[2u8; 32]);
        assert_eq!(s2.authenticate(b"x").len(), 32);
        assert_ne!(s1.hmac_key(), s2.hmac_key());
        assert!(s2.decrypt(&ct[..20]).is_err());
    }

    #[test]
    fn attested_credential_parses_cose_key() {
        let mut auth_data = vec![0u8; 32];
        auth_data.push(0x41); // UP | AT
        auth_data.extend_from_slice(&[0, 0, 0, 1]);
        auth_data.extend_from_slice(&[0xAA; 16]);
        auth_data.extend_from_slice(&3u16.to_be_bytes());
        auth_data.extend_from_slice(&[1, 2, 3]);
        auth_data.extend(encode(&map(vec![
            (int(1), int(1)),
            (int(3), int(-8)),
            (int(-1), int(6)),
            (int(-2), bytes(&[5u8; 32])),
        ])));
        auth_data.extend_from_slice(&[0xA0]); // empty extensions map
        let c = attested_credential(&auth_data).unwrap();
        assert_eq!(c.id, vec![1, 2, 3]);
        assert_eq!(c.public_key, CosePublicKey::Ed25519([5u8; 32]));

        // Missing AT flag.
        auth_data[32] = 0x01;
        assert!(attested_credential(&auth_data).is_err());
    }
}

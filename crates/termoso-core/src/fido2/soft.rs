//! In-memory CTAP2 authenticator – a *test double* for the client in
//! [`super::ctap`] and the framing in [`super::hid`] / [`super::nfc`]. It
//! behaves like a PIN-capable CTAP 2.1 token (ES256 + EdDSA, resident keys,
//! credential management, PIN protocols 1 and 2) so the whole SSH path can
//! be exercised without hardware. Not for production use: its keys live in
//! process memory.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ciborium::Value;
use ed25519_dalek::Signer as _;
use p256::elliptic_curve::sec1::ToEncodedPoint;
use sha2::{Digest, Sha256};

use super::Fido2Error;
use super::ctap::{
    self, CtapTransport, Field, SharedSecret, as_bytes, as_i64, as_u64, bytes, cose_public_key,
    encode, int, map, text,
};
use super::hid::{HidPackets, PACKET_SIZE, Reassembler, frame};
use super::nfc::{Apdu, FIDO_AID};

const STATUS_OK: u8 = 0x00;
const STATUS_INVALID_CBOR: u8 = 0x12;
const STATUS_MISSING_PARAMETER: u8 = 0x14;
const STATUS_UNSUPPORTED_ALGORITHM: u8 = 0x26;
const STATUS_OPERATION_DENIED: u8 = 0x27;
const STATUS_NO_CREDENTIALS: u8 = 0x2E;
const STATUS_USER_ACTION_TIMEOUT: u8 = 0x2F;
const STATUS_PIN_INVALID: u8 = 0x31;
const STATUS_PIN_BLOCKED: u8 = 0x32;
const STATUS_PIN_AUTH_INVALID: u8 = 0x33;
const STATUS_PIN_NOT_SET: u8 = 0x35;
const STATUS_PIN_REQUIRED: u8 = 0x36;
const STATUS_INVALID_COMMAND: u8 = 0x01;
const STATUS_INVALID_PARAMETER: u8 = 0x02;

/// What the "user" does when the token asks for a touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    /// Touches promptly.
    Accept,
    /// Never touches.
    Timeout,
    /// Cancels.
    Deny,
}

enum Key {
    P256(p256::ecdsa::SigningKey),
    Ed25519(ed25519_dalek::SigningKey),
}

impl Key {
    fn cose(&self) -> Value {
        match self {
            Key::P256(k) => {
                let p = k.verifying_key().to_encoded_point(false);
                map(vec![
                    (int(1), int(2)),
                    (int(3), int(-7)),
                    (int(-1), int(1)),
                    (int(-2), bytes(p.x().expect("uncompressed"))),
                    (int(-3), bytes(p.y().expect("uncompressed"))),
                ])
            }
            Key::Ed25519(k) => map(vec![
                (int(1), int(1)),
                (int(3), int(-8)),
                (int(-1), int(6)),
                (int(-2), bytes(k.verifying_key().as_bytes())),
            ]),
        }
    }

    fn sign(&self, msg: &[u8]) -> Vec<u8> {
        match self {
            Key::P256(k) => {
                let sig: p256::ecdsa::DerSignature = k.sign(msg);
                sig.as_bytes().to_vec()
            }
            Key::Ed25519(k) => k.sign(msg).to_bytes().to_vec(),
        }
    }
}

struct Stored {
    rp_id: String,
    user_id: Vec<u8>,
    user_name: String,
    key: Key,
    resident: bool,
}

/// Knobs and observable state of the token.
#[derive(Debug, Clone)]
pub struct Config {
    /// Client PIN; `None` = no PIN set.
    pub pin: Option<String>,
    /// PIN/UV auth protocols to advertise.
    pub pin_protocols: Vec<u64>,
    /// Whether to accept `EdDSA`.
    pub ed25519: bool,
    /// Advertise `credMgmt` (CTAP 2.1) – otherwise `credentialMgmtPreview`.
    pub cred_mgmt: bool,
    /// Advertise `pinUvAuthToken` (permissions).
    pub pin_uv_auth_token: bool,
    /// User reaction to touch requests.
    pub touch: Touch,
    /// Accept extended-length APDUs over NFC.
    pub nfc_extended_length: bool,
    /// Answer every NFC request with `9100` first (still working) so the
    /// client has to poll `NFCCTAP_GETRESPONSE`.
    pub nfc_processing: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pin: Some("123456".into()),
            pin_protocols: vec![2, 1],
            ed25519: true,
            cred_mgmt: true,
            pin_uv_auth_token: true,
            touch: Touch::Accept,
            nfc_extended_length: false,
            nfc_processing: false,
        }
    }
}

struct State {
    config: Config,
    creds: BTreeMap<Vec<u8>, Stored>,
    counter: u32,
    pin_retries: i32,
    /// Ephemeral key agreement secret (regenerated per `getKeyAgreement`).
    agreement: Option<p256::ecdh::EphemeralSecret>,
    /// Issued PIN tokens with their protocol and permissions.
    tokens: Vec<(u64, Vec<u8>, u8)>,
    /// Enumeration cursors.
    rp_cursor: Vec<String>,
    cred_cursor: Vec<Vec<u8>>,
    /// Requests seen, for tests.
    log: Vec<u8>,
    touches: usize,
}

/// The authenticator. Clone handles share one token.
#[derive(Clone)]
pub struct SoftToken {
    state: Arc<Mutex<State>>,
}

impl Default for SoftToken {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl SoftToken {
    /// New token with `config`.
    pub fn new(config: Config) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                config,
                creds: BTreeMap::new(),
                counter: 0,
                pin_retries: 8,
                agreement: None,
                tokens: Vec::new(),
                rp_cursor: Vec::new(),
                cred_cursor: Vec::new(),
                log: Vec::new(),
                touches: 0,
            })),
        }
    }

    /// Change behaviour between operations.
    pub fn configure(&self, f: impl FnOnce(&mut Config)) {
        f(&mut self.state.lock().unwrap().config);
    }

    /// Commands received so far (first byte of each request).
    pub fn commands(&self) -> Vec<u8> {
        self.state.lock().unwrap().log.clone()
    }

    /// How many times the user was asked to touch.
    pub fn touches(&self) -> usize {
        self.state.lock().unwrap().touches
    }

    /// Stored credentials (resident or not).
    pub fn credential_count(&self) -> usize {
        self.state.lock().unwrap().creds.len()
    }

    /// Remaining PIN attempts.
    pub fn pin_retries(&self) -> i32 {
        self.state.lock().unwrap().pin_retries
    }

    /// Process one CTAP message (`cmd ‖ CBOR`) → `status ‖ CBOR`.
    pub fn handle(&self, payload: &[u8]) -> Vec<u8> {
        let mut st = self.state.lock().unwrap();
        let Some((&cmd, body)) = payload.split_first() else {
            return vec![STATUS_INVALID_CBOR];
        };
        st.log.push(cmd);
        let params = if body.is_empty() {
            None
        } else {
            match ctap::decode(body) {
                Ok(v) => Some(v),
                Err(_) => return vec![STATUS_INVALID_CBOR],
            }
        };
        let result = match cmd {
            ctap::CMD_GET_INFO => Ok(Some(st.get_info())),
            ctap::CMD_CLIENT_PIN => st.client_pin(params.as_ref()),
            ctap::CMD_MAKE_CREDENTIAL => st.make_credential(params.as_ref()),
            ctap::CMD_GET_ASSERTION => st.get_assertion(params.as_ref()),
            ctap::CMD_CRED_MGMT | ctap::CMD_CRED_MGMT_PREVIEW => {
                let expected = if st.config.cred_mgmt {
                    ctap::CMD_CRED_MGMT
                } else {
                    ctap::CMD_CRED_MGMT_PREVIEW
                };
                if cmd == expected {
                    st.cred_mgmt(params.as_ref())
                } else {
                    Err(STATUS_INVALID_COMMAND)
                }
            }
            _ => Err(STATUS_INVALID_COMMAND),
        };
        match result {
            Ok(None) => vec![STATUS_OK],
            Ok(Some(v)) => {
                let mut out = vec![STATUS_OK];
                out.extend(encode(&v));
                out
            }
            Err(status) => vec![status],
        }
    }

    /// The token as a direct [`CtapTransport`] (no framing).
    pub fn direct(&self) -> Direct {
        Direct(self.clone())
    }

    /// The token behind a CTAPHID "cable" – exercises [`super::hid`].
    pub fn hid(&self) -> HidWire {
        HidWire {
            token: self.clone(),
            cid: 0,
            broadcast: Reassembler::new(0xFFFF_FFFF),
            asm: Reassembler::new(0),
            outbox: Vec::new(),
            drop_next_read: false,
        }
    }

    /// The token as an NFC tag – exercises [`super::nfc`].
    pub fn nfc(&self) -> NfcTag {
        NfcTag {
            token: self.clone(),
            selected: false,
            pending: Vec::new(),
            chained: Vec::new(),
        }
    }
}

type Reply = Result<Option<Value>, u8>;

impl State {
    fn get_info(&self) -> Value {
        let mut options = vec![
            (text("rk"), Value::Bool(true)),
            (text("up"), Value::Bool(true)),
            (text("clientPin"), Value::Bool(self.config.pin.is_some())),
        ];
        options.push((
            text(if self.config.cred_mgmt {
                "credMgmt"
            } else {
                "credentialMgmtPreview"
            }),
            Value::Bool(true),
        ));
        if self.config.pin_uv_auth_token {
            options.push((text("pinUvAuthToken"), Value::Bool(true)));
        }
        let mut algs = vec![map(vec![
            (text("alg"), int(-7)),
            (text("type"), text("public-key")),
        ])];
        if self.config.ed25519 {
            algs.push(map(vec![
                (text("alg"), int(-8)),
                (text("type"), text("public-key")),
            ]));
        }
        map(vec![
            (
                int(1),
                Value::Array(vec![text("FIDO_2_0"), text("FIDO_2_1")]),
            ),
            (int(2), Value::Array(vec![text("credProtect")])),
            (int(3), bytes(&[0x42; 16])),
            (int(4), map(options)),
            (int(5), int(1200)),
            (
                int(6),
                Value::Array(
                    self.config
                        .pin_protocols
                        .iter()
                        .map(|p| int(*p as i64))
                        .collect(),
                ),
            ),
            (int(10), Value::Array(algs)),
        ])
    }

    fn shared_secret(&mut self, protocol: u64, peer: &Value) -> Result<SharedSecret, u8> {
        let super::CosePublicKey::P256 { x, y } =
            cose_public_key(peer).map_err(|_| STATUS_INVALID_PARAMETER)?
        else {
            return Err(STATUS_INVALID_PARAMETER);
        };
        let mut sec1 = vec![0x04];
        sec1.extend_from_slice(&x);
        sec1.extend_from_slice(&y);
        let peer = p256::PublicKey::from_sec1_bytes(&sec1).map_err(|_| STATUS_INVALID_PARAMETER)?;
        let ours = self.agreement.as_ref().ok_or(STATUS_MISSING_PARAMETER)?;
        let z = ours.diffie_hellman(&peer);
        Ok(SharedSecret::new(protocol, z.raw_secret_bytes()))
    }

    fn client_pin(&mut self, params: Option<&Value>) -> Reply {
        let p = params.ok_or(STATUS_MISSING_PARAMETER)?;
        let protocol = p.field(1).and_then(as_u64).unwrap_or(1);
        if !self.config.pin_protocols.contains(&protocol) {
            return Err(STATUS_INVALID_PARAMETER);
        }
        let sub = p
            .field(2)
            .and_then(as_u64)
            .ok_or(STATUS_MISSING_PARAMETER)?;
        match sub as u8 {
            0x01 => Ok(Some(map(vec![(int(3), int(self.pin_retries as i64))]))),
            0x02 => {
                let secret = p256::ecdh::EphemeralSecret::random(&mut rand_core::OsRng);
                let point = secret.public_key().to_encoded_point(false);
                self.agreement = Some(secret);
                Ok(Some(map(vec![(
                    int(1),
                    map(vec![
                        (int(1), int(2)),
                        (int(3), int(-25)),
                        (int(-1), int(1)),
                        (int(-2), bytes(point.x().expect("uncompressed"))),
                        (int(-3), bytes(point.y().expect("uncompressed"))),
                    ]),
                )])))
            }
            0x05 | 0x09 => {
                let Some(pin) = self.config.pin.clone() else {
                    return Err(STATUS_PIN_NOT_SET);
                };
                if self.pin_retries <= 0 {
                    return Err(STATUS_PIN_BLOCKED);
                }
                let peer = p.field(3).ok_or(STATUS_MISSING_PARAMETER)?;
                let secret = self.shared_secret(protocol, peer)?;
                let enc =
                    as_bytes(p.field(6), "pinHashEnc").map_err(|_| STATUS_MISSING_PARAMETER)?;
                let got = secret.decrypt(&enc).map_err(|_| STATUS_INVALID_PARAMETER)?;
                let want = Sha256::digest(pin.as_bytes());
                if got.as_slice() != &want[..16] {
                    self.pin_retries -= 1;
                    self.agreement = None;
                    return Err(if self.pin_retries == 0 {
                        STATUS_PIN_BLOCKED
                    } else {
                        STATUS_PIN_INVALID
                    });
                }
                self.pin_retries = 8;
                let permissions = if sub == 0x09 {
                    if !self.config.pin_uv_auth_token {
                        return Err(STATUS_INVALID_COMMAND);
                    }
                    p.field(9)
                        .and_then(as_u64)
                        .ok_or(STATUS_MISSING_PARAMETER)? as u8
                } else {
                    0xFF
                };
                let mut token = vec![0u8; 32];
                rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut token);
                self.tokens.push((protocol, token.clone(), permissions));
                Ok(Some(map(vec![(int(2), bytes(&secret.encrypt(&token)))])))
            }
            _ => Err(STATUS_INVALID_COMMAND),
        }
    }

    /// Verify `pinUvAuthParam` over `msg`; `Ok(true)` when a valid token was
    /// presented, `Ok(false)` when none was.
    fn check_pin_auth(
        &self,
        param: Option<&Value>,
        protocol: Option<u64>,
        msg: &[u8],
        permission: u8,
    ) -> Result<bool, u8> {
        let Some(param) = param.and_then(Value::as_bytes) else {
            return Ok(false);
        };
        let protocol = protocol.ok_or(STATUS_MISSING_PARAMETER)?;
        let ok = self.tokens.iter().any(|(proto, token, perms)| {
            *proto == protocol
                && perms & permission != 0
                && ctap::authenticate(protocol, token, msg) == *param
        });
        if ok {
            Ok(true)
        } else {
            Err(STATUS_PIN_AUTH_INVALID)
        }
    }

    fn touch(&mut self) -> Result<(), u8> {
        self.touches += 1;
        match self.config.touch {
            Touch::Accept => Ok(()),
            Touch::Timeout => Err(STATUS_USER_ACTION_TIMEOUT),
            Touch::Deny => Err(STATUS_OPERATION_DENIED),
        }
    }

    fn auth_data(&mut self, rp_id: &str, flags: u8, attested: Option<(&[u8], &Key)>) -> Vec<u8> {
        self.counter += 1;
        let mut out = Sha256::digest(rp_id.as_bytes()).to_vec();
        out.push(flags);
        out.extend_from_slice(&self.counter.to_be_bytes());
        if let Some((id, key)) = attested {
            out.extend_from_slice(&[0x42; 16]);
            out.extend_from_slice(&(id.len() as u16).to_be_bytes());
            out.extend_from_slice(id);
            out.extend(encode(&key.cose()));
        }
        out
    }

    fn make_credential(&mut self, params: Option<&Value>) -> Reply {
        let p = params.ok_or(STATUS_MISSING_PARAMETER)?;
        let client_data_hash =
            as_bytes(p.field(1), "clientDataHash").map_err(|_| STATUS_MISSING_PARAMETER)?;
        let rp_id = p
            .field(2)
            .and_then(|rp| rp.field_text("id"))
            .and_then(Value::as_text)
            .ok_or(STATUS_MISSING_PARAMETER)?
            .to_string();
        let user = p.field(3).ok_or(STATUS_MISSING_PARAMETER)?;
        let user_id =
            as_bytes(user.field_text("id"), "user.id").map_err(|_| STATUS_MISSING_PARAMETER)?;
        let user_name = user
            .field_text("name")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_string();
        let algs: Vec<i64> = p
            .field(4)
            .and_then(Value::as_array)
            .ok_or(STATUS_MISSING_PARAMETER)?
            .iter()
            .filter_map(|e| e.field_text("alg").and_then(as_i64))
            .collect();
        let resident = p
            .field(7)
            .and_then(|o| o.field_text("rk"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let verified = self.check_pin_auth(
            p.field(8),
            p.field(9).and_then(as_u64),
            &client_data_hash,
            0x01,
        )?;
        if self.config.pin.is_some() && !verified {
            return Err(STATUS_PIN_REQUIRED);
        }
        // Tokens pick the first algorithm they support from the list.
        let key = algs
            .iter()
            .find_map(|alg| match alg {
                -7 => Some(Key::P256(p256::ecdsa::SigningKey::random(
                    &mut rand_core::OsRng,
                ))),
                -8 if self.config.ed25519 => {
                    let mut seed = [0u8; 32];
                    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut seed);
                    Some(Key::Ed25519(ed25519_dalek::SigningKey::from_bytes(&seed)))
                }
                _ => None,
            })
            .ok_or(STATUS_UNSUPPORTED_ALGORITHM)?;
        self.touch()?;

        let mut id = vec![0u8; 48];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut id);
        let mut flags = 0x01 | 0x40;
        if verified {
            flags |= 0x04;
        }
        let auth_data = self.auth_data(&rp_id, flags, Some((&id, &key)));
        self.creds.insert(
            id,
            Stored {
                rp_id,
                user_id,
                user_name,
                key,
                resident,
            },
        );
        Ok(Some(map(vec![
            (int(1), text("none")),
            (int(2), bytes(&auth_data)),
            (int(3), map(vec![])),
        ])))
    }

    fn get_assertion(&mut self, params: Option<&Value>) -> Reply {
        let p = params.ok_or(STATUS_MISSING_PARAMETER)?;
        let rp_id = p
            .field(1)
            .and_then(Value::as_text)
            .ok_or(STATUS_MISSING_PARAMETER)?
            .to_string();
        let client_data_hash =
            as_bytes(p.field(2), "clientDataHash").map_err(|_| STATUS_MISSING_PARAMETER)?;
        let allow: Vec<Vec<u8>> = p
            .field(3)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.field_text("id").and_then(Value::as_bytes).cloned())
                    .collect()
            })
            .unwrap_or_default();
        let up = p
            .field(5)
            .and_then(|o| o.field_text("up"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let verified = self.check_pin_auth(
            p.field(6),
            p.field(7).and_then(as_u64),
            &client_data_hash,
            0x02,
        )?;

        let id = if allow.is_empty() {
            self.creds
                .iter()
                .find(|(_, c)| c.rp_id == rp_id && c.resident)
                .map(|(id, _)| id.clone())
        } else {
            allow
                .into_iter()
                .find(|id| self.creds.get(id).is_some_and(|c| c.rp_id == rp_id))
        };
        let id = id.ok_or(STATUS_NO_CREDENTIALS)?;
        if up {
            self.touch()?;
        }
        let mut flags = 0u8;
        if up {
            flags |= 0x01;
        }
        if verified {
            flags |= 0x04;
        }
        let auth_data = self.auth_data(&rp_id, flags, None);
        let cred = &self.creds[&id];
        let mut msg = auth_data.clone();
        msg.extend_from_slice(&client_data_hash);
        let signature = cred.key.sign(&msg);
        Ok(Some(map(vec![
            (
                int(1),
                map(vec![
                    (text("id"), bytes(&id)),
                    (text("type"), text("public-key")),
                ]),
            ),
            (int(2), bytes(&auth_data)),
            (int(3), bytes(&signature)),
            (
                int(4),
                map(vec![
                    (text("id"), bytes(&cred.user_id)),
                    (text("name"), text(&cred.user_name)),
                ]),
            ),
        ])))
    }

    fn cred_mgmt(&mut self, params: Option<&Value>) -> Reply {
        let p = params.ok_or(STATUS_MISSING_PARAMETER)?;
        let sub = p
            .field(1)
            .and_then(as_u64)
            .ok_or(STATUS_MISSING_PARAMETER)? as u8;
        let sub_params = p.field(2);
        if matches!(sub, 0x02 | 0x04) {
            let mut msg = vec![sub];
            if let Some(sp) = sub_params {
                msg.extend(encode(sp));
            }
            if !self.check_pin_auth(p.field(4), p.field(3).and_then(as_u64), &msg, 0x04)? {
                return Err(STATUS_PIN_REQUIRED);
            }
        }
        let rp_entry = |rp: &str, total: Option<usize>| {
            let mut m = vec![
                (int(3), map(vec![(text("id"), text(rp))])),
                (int(4), bytes(&Sha256::digest(rp.as_bytes()))),
            ];
            if let Some(t) = total {
                m.push((int(5), int(t as i64)));
            }
            map(m)
        };
        match sub {
            0x02 => {
                let mut rps: Vec<String> = self
                    .creds
                    .values()
                    .filter(|c| c.resident)
                    .map(|c| c.rp_id.clone())
                    .collect();
                rps.sort();
                rps.dedup();
                if rps.is_empty() {
                    return Err(STATUS_NO_CREDENTIALS);
                }
                let total = rps.len();
                let first = rps.remove(0);
                self.rp_cursor = rps;
                Ok(Some(rp_entry(&first, Some(total))))
            }
            0x03 => {
                if self.rp_cursor.is_empty() {
                    return Err(STATUS_NO_CREDENTIALS);
                }
                let rp = self.rp_cursor.remove(0);
                Ok(Some(rp_entry(&rp, None)))
            }
            0x04 | 0x05 => {
                if sub == 0x04 {
                    let hash = sub_params
                        .and_then(|sp| sp.field(1))
                        .and_then(Value::as_bytes)
                        .ok_or(STATUS_MISSING_PARAMETER)?
                        .clone();
                    let mut ids: Vec<Vec<u8>> = self
                        .creds
                        .iter()
                        .filter(|(_, c)| {
                            c.resident && Sha256::digest(c.rp_id.as_bytes()).as_slice() == hash
                        })
                        .map(|(id, _)| id.clone())
                        .collect();
                    if ids.is_empty() {
                        return Err(STATUS_NO_CREDENTIALS);
                    }
                    ids.reverse();
                    self.cred_cursor = ids;
                }
                let id = self.cred_cursor.pop().ok_or(STATUS_NO_CREDENTIALS)?;
                let total = self.cred_cursor.len() + 1;
                let c = &self.creds[&id];
                let mut m = vec![
                    (
                        int(6),
                        map(vec![
                            (text("id"), bytes(&c.user_id)),
                            (text("name"), text(&c.user_name)),
                        ]),
                    ),
                    (
                        int(7),
                        map(vec![
                            (text("id"), bytes(&id)),
                            (text("type"), text("public-key")),
                        ]),
                    ),
                    (int(8), c.key.cose()),
                ];
                if sub == 0x04 {
                    m.push((int(9), int(total as i64)));
                }
                Ok(Some(map(m)))
            }
            _ => Err(STATUS_INVALID_COMMAND),
        }
    }
}

/// [`CtapTransport`] straight into the token.
pub struct Direct(SoftToken);

impl CtapTransport for Direct {
    fn cbor(&mut self, payload: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        Ok(self.0.handle(payload))
    }
}

/// [`HidPackets`] that behave like the token's USB interface: INIT on the
/// broadcast channel hands out a channel (any number of times, like a real
/// token a client re-opens), CBOR requests are reassembled, replies framed
/// back.
pub struct HidWire {
    token: SoftToken,
    cid: u32,
    broadcast: Reassembler,
    asm: Reassembler,
    outbox: Vec<Vec<u8>>,
    drop_next_read: bool,
}

impl HidWire {
    /// Make the next `read` time out once (simulates a slow token).
    pub fn stall_once(&mut self) {
        self.drop_next_read = true;
    }
}

impl HidPackets for HidWire {
    fn write(&mut self, packet: &[u8]) -> Result<(), Fido2Error> {
        assert_eq!(packet.len(), PACKET_SIZE, "reports are 64 bytes");
        let cid = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        if cid != 0xFFFF_FFFF && cid != self.cid {
            // Unknown channel: CTAPHID_ERROR INVALID_CHANNEL.
            self.outbox.extend(frame(cid, 0x3F, &[0x0B]));
            return Ok(());
        }
        let asm = if cid == 0xFFFF_FFFF {
            &mut self.broadcast
        } else {
            &mut self.asm
        };
        let Some((cmd, data)) = asm.push(packet)? else {
            return Ok(());
        };
        match cmd {
            0x06 => {
                self.cid = 0x1234_5678;
                let mut resp = data[..8].to_vec();
                resp.extend_from_slice(&self.cid.to_be_bytes());
                resp.extend_from_slice(&[2, 1, 0, 0, 0x04]); // proto 2, v1.0.0, CBOR capable
                self.outbox.extend(frame(cid, 0x06, &resp));
                self.asm = Reassembler::new(self.cid);
            }
            0x10 => {
                // A keep-alive first, as real tokens send while waiting.
                self.outbox.extend(frame(self.cid, 0x3B, &[0x02]));
                let reply = self.token.handle(&data);
                self.outbox.extend(frame(self.cid, 0x10, &reply));
            }
            0x11 => {}
            other => self.outbox.extend(frame(self.cid, 0x3F, &[other])),
        }
        Ok(())
    }

    fn read(&mut self, _timeout: std::time::Duration) -> Result<Option<Vec<u8>>, Fido2Error> {
        if self.drop_next_read {
            self.drop_next_read = false;
            return Ok(None);
        }
        if self.outbox.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.outbox.remove(0)))
    }
}

/// [`Apdu`] that behaves like the token's NFC applet, including command
/// chaining and `61xx` response chaining.
pub struct NfcTag {
    token: SoftToken,
    selected: bool,
    pending: Vec<u8>,
    chained: Vec<u8>,
}

impl Apdu for NfcTag {
    fn transceive(&mut self, apdu: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        if apdu.len() < 4 {
            return Ok(vec![0x67, 0x00]);
        }
        let (cla, ins, _p1, _p2) = (apdu[0], apdu[1], apdu[2], apdu[3]);
        let (data, _le) = parse_body(&apdu[4..]);
        match (cla, ins) {
            (0x00, 0xA4) => {
                self.selected = data == FIDO_AID;
                if self.selected {
                    let mut r = b"FIDO_2_0".to_vec();
                    r.extend_from_slice(&[0x90, 0x00]);
                    Ok(r)
                } else {
                    Ok(vec![0x6A, 0x82])
                }
            }
            (0x00, 0xC0) | (0x80, 0x11) => Ok(self.next_chunk()),
            (0x80 | 0x90, 0x10) if self.selected => {
                self.chained.extend_from_slice(data);
                if cla == 0x90 {
                    return Ok(vec![0x90, 0x00]);
                }
                let request = std::mem::take(&mut self.chained);
                self.pending = self.token.handle(&request);
                if self.token.state.lock().unwrap().config.nfc_processing {
                    return Ok(vec![0x91, 0x00]);
                }
                Ok(self.next_chunk())
            }
            _ => Ok(vec![0x6D, 0x00]),
        }
    }

    fn extended_length(&self) -> bool {
        self.token.state.lock().unwrap().config.nfc_extended_length
    }
}

impl NfcTag {
    fn next_chunk(&mut self) -> Vec<u8> {
        let n = self.pending.len().min(SHORT_CHUNK);
        let mut r: Vec<u8> = self.pending.drain(..n).collect();
        r.extend_from_slice(&status_for(self.pending.len()));
        r
    }
}

/// Response chunk size the tag uses (small, to force `61xx` chaining).
const SHORT_CHUNK: usize = 200;

fn status_for(remaining: usize) -> [u8; 2] {
    if remaining == 0 {
        [0x90, 0x00]
    } else {
        [0x61, remaining.min(SHORT_CHUNK) as u8]
    }
}

/// Split `Lc ‖ data ‖ Le` (short or extended) out of an APDU body.
fn parse_body(body: &[u8]) -> (&[u8], usize) {
    match body {
        [] => (&[], 0),
        [0x00, hi, lo, rest @ ..] if rest.len() >= u16::from_be_bytes([*hi, *lo]) as usize => {
            let lc = u16::from_be_bytes([*hi, *lo]) as usize;
            (&rest[..lc], 0)
        }
        [lc, rest @ ..] => {
            let lc = (*lc as usize).min(rest.len());
            (&rest[..lc], 0)
        }
    }
}

/// [`SkBackend`](super::SkBackend) over the soft token – lets the SSH auth
/// path be tested end to end. `wire` picks the framing to go through.
pub struct SoftBackend {
    token: SoftToken,
    wire: Wire,
}

/// Which adapter a [`SoftBackend`] speaks through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// Straight CBOR.
    Direct,
    /// CTAPHID framing.
    Hid,
    /// ISO 7816 APDUs.
    Nfc,
}

impl SoftBackend {
    /// Backend reaching `token` through `wire`.
    pub fn new(token: SoftToken, wire: Wire) -> Self {
        Self { token, wire }
    }

    /// Run `f` against an authenticator opened over the chosen wire.
    pub fn with<R>(
        &self,
        f: impl FnOnce(&mut dyn CtapAny) -> crate::error::Result<R>,
    ) -> crate::error::Result<R> {
        match self.wire {
            Wire::Direct => {
                let mut a = ctap::Authenticator::open(self.token.direct())?;
                f(&mut a)
            }
            Wire::Hid => {
                let mut a =
                    ctap::Authenticator::open(super::hid::HidTransport::open(self.token.hid())?)?;
                f(&mut a)
            }
            Wire::Nfc => {
                let mut a =
                    ctap::Authenticator::open(super::nfc::NfcTransport::open(self.token.nfc())?)?;
                f(&mut a)
            }
        }
    }
}

/// Object-safe view of an [`ctap::Authenticator`] over any transport.
pub trait CtapAny {
    /// [`ctap::generate_with`].
    fn generate(
        &mut self,
        opts: &super::GenerateOptions,
    ) -> crate::error::Result<crate::keys::KeyMaterial>;
    /// [`ctap::sign_with`].
    fn sign(
        &mut self,
        key: &russh::keys::ssh_key::PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> crate::error::Result<Vec<u8>>;
    /// [`ctap::load_resident_with`].
    fn load_resident(
        &mut self,
        pin: &str,
        passphrase: Option<&str>,
    ) -> crate::error::Result<Vec<crate::keys::KeyMaterial>>;
    /// [`ctap::Authenticator::info`].
    fn info(&self) -> &ctap::Info;
}

impl<T: CtapTransport> CtapAny for ctap::Authenticator<T> {
    fn generate(
        &mut self,
        opts: &super::GenerateOptions,
    ) -> crate::error::Result<crate::keys::KeyMaterial> {
        ctap::generate_with(self, opts)
    }

    fn sign(
        &mut self,
        key: &russh::keys::ssh_key::PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> crate::error::Result<Vec<u8>> {
        ctap::sign_with(self, key, data, pin)
    }

    fn load_resident(
        &mut self,
        pin: &str,
        passphrase: Option<&str>,
    ) -> crate::error::Result<Vec<crate::keys::KeyMaterial>> {
        ctap::load_resident_with(self, pin, passphrase)
    }

    fn info(&self) -> &ctap::Info {
        ctap::Authenticator::info(self)
    }
}

impl super::SkBackend for SoftBackend {
    fn sign(
        &self,
        key: &russh::keys::ssh_key::PrivateKey,
        data: &[u8],
        pin: Option<&str>,
    ) -> crate::error::Result<Vec<u8>> {
        self.with(|a| a.sign(key, data, pin))
    }
}

#[cfg(test)]
mod tests {
    use russh::keys::signature::Verifier;
    use russh::keys::ssh_encoding::Decode;
    use russh::keys::ssh_key::Algorithm;

    use super::super::{FLAG_RESIDENT, FLAG_USER_PRESENCE, FLAG_USER_VERIFICATION, SkAlgorithm};
    use super::*;
    use crate::error::CoreError;
    use crate::ssh::load_private_key;

    fn opts(alg: SkAlgorithm) -> super::super::GenerateOptions {
        super::super::GenerateOptions {
            device: None,
            algorithm: alg,
            application: None,
            resident: false,
            user_presence: true,
            user_verification: false,
            pin: Some(zeroize::Zeroizing::new("123456".into())),
            user: None,
            comment: "test@soft".into(),
            passphrase: None,
        }
    }

    fn sk_err(e: CoreError) -> Fido2Error {
        match e {
            CoreError::Fido2(f) => f,
            other => panic!("expected a FIDO2 error, got {other:?}"),
        }
    }

    /// Generate, sign and verify with OpenSSH's own `sk-*` verifier over
    /// every wire – the signature must be what `sshd` would accept.
    fn round_trip(wire: Wire, alg: SkAlgorithm) {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), wire);
        let material = backend.with(|a| a.generate(&opts(alg))).unwrap();
        let key = load_private_key(&material.private_key, None).unwrap();
        assert_eq!(key.algorithm(), alg.ssh_algorithm());
        assert!(
            material
                .public_key
                .starts_with(alg.ssh_algorithm().as_str())
        );
        assert_eq!(token.credential_count(), 1);
        assert_eq!(token.touches(), 1);

        let data = b"exchange hash and userauth request";
        let blob = backend.with(|a| a.sign(&key, data, None)).unwrap();
        assert_eq!(token.touches(), 2);
        let sig = russh::keys::ssh_key::Signature::decode(&mut &blob[..]).unwrap();
        Verifier::verify(key.public_key().key_data(), data, &sig).unwrap();
        // A different message must not verify.
        assert!(Verifier::verify(key.public_key().key_data(), b"other", &sig).is_err());
    }

    #[test]
    fn direct_ed25519() {
        round_trip(Wire::Direct, SkAlgorithm::Ed25519);
    }

    #[test]
    fn direct_p256() {
        round_trip(Wire::Direct, SkAlgorithm::EcdsaP256);
    }

    #[test]
    fn hid_ed25519_and_p256() {
        round_trip(Wire::Hid, SkAlgorithm::Ed25519);
        round_trip(Wire::Hid, SkAlgorithm::EcdsaP256);
    }

    #[test]
    fn nfc_ed25519_and_p256() {
        round_trip(Wire::Nfc, SkAlgorithm::Ed25519);
        round_trip(Wire::Nfc, SkAlgorithm::EcdsaP256);
    }

    #[test]
    fn nfc_extended_length_and_processing_poll() {
        let token = SoftToken::new(Config {
            nfc_extended_length: true,
            nfc_processing: true,
            ..Config::default()
        });
        let backend = SoftBackend::new(token.clone(), Wire::Nfc);
        let m = backend
            .with(|a| a.generate(&opts(SkAlgorithm::EcdsaP256)))
            .unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        backend.with(|a| a.sign(&key, b"x", None)).unwrap();
        assert_eq!(token.touches(), 2);
    }

    #[test]
    fn hid_reassembles_large_messages_and_survives_a_stall() {
        // getInfo response and makeCredential requests span several reports.
        let token = SoftToken::default();
        let mut wire = token.hid();
        wire.stall_once();
        let mut auth =
            ctap::Authenticator::open(super::super::hid::HidTransport::open(wire).unwrap())
                .unwrap();
        assert!(auth.info().has("rk"));
        assert_eq!(auth.info().pin_protocols, vec![2, 1]);
        let mut o = opts(SkAlgorithm::Ed25519);
        o.resident = true;
        o.user = Some("alice".into());
        o.comment = "x".repeat(300); // comment is local only; request stays small
        let m = ctap::generate_with(&mut auth, &o).unwrap();
        assert!(m.public_key.contains(&"x".repeat(300)));
        // INIT, getInfo, clientPIN×2 (agreement + token), makeCredential
        assert_eq!(
            token.commands(),
            vec![
                ctap::CMD_GET_INFO,
                ctap::CMD_CLIENT_PIN,
                ctap::CMD_CLIENT_PIN,
                ctap::CMD_MAKE_CREDENTIAL
            ]
        );
    }

    #[test]
    fn pin_protocol_1_only_token() {
        let token = SoftToken::new(Config {
            pin_protocols: vec![1],
            pin_uv_auth_token: false,
            cred_mgmt: false,
            ..Config::default()
        });
        let backend = SoftBackend::new(token.clone(), Wire::Direct);
        let mut o = opts(SkAlgorithm::EcdsaP256);
        o.resident = true;
        let m = backend.with(|a| a.generate(&o)).unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        backend.with(|a| a.sign(&key, b"data", None)).unwrap();
        // credentialMgmtPreview path
        let loaded = backend.with(|a| a.load_resident("123456", None)).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].public_key,
            m.public_key.replace(" test@soft", " termoso")
        );
    }

    #[test]
    fn pin_errors_are_typed() {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), Wire::Hid);

        // No PIN given but token has one → the token demands it.
        let mut o = opts(SkAlgorithm::Ed25519);
        o.pin = None;
        assert_eq!(
            sk_err(backend.with(|a| a.generate(&o)).unwrap_err()),
            Fido2Error::PinRequired
        );

        // Wrong PIN → invalid with retries reported.
        o.pin = Some(zeroize::Zeroizing::new("000000".into()));
        assert_eq!(
            sk_err(backend.with(|a| a.generate(&o)).unwrap_err()),
            Fido2Error::PinInvalid { retries: Some(7) }
        );
        assert_eq!(token.pin_retries(), 7);

        // Right PIN restores the counter.
        o.pin = Some(zeroize::Zeroizing::new("123456".into()));
        backend.with(|a| a.generate(&o)).unwrap();
        assert_eq!(token.pin_retries(), 8);

        // Exhaust → blocked.
        for _ in 0..7 {
            o.pin = Some(zeroize::Zeroizing::new("000000".into()));
            let _ = backend.with(|a| a.generate(&o));
        }
        assert_eq!(
            sk_err(backend.with(|a| a.generate(&o)).unwrap_err()),
            Fido2Error::PinBlocked
        );

        // Token without a PIN.
        let bare = SoftToken::new(Config {
            pin: None,
            ..Config::default()
        });
        let backend = SoftBackend::new(bare.clone(), Wire::Direct);
        assert_eq!(
            sk_err(backend.with(|a| a.generate(&o)).unwrap_err()),
            Fido2Error::PinNotSet
        );
        o.pin = None;
        let m = backend.with(|a| a.generate(&o)).unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        backend.with(|a| a.sign(&key, b"d", None)).unwrap();
    }

    #[test]
    fn verify_required_key_needs_pin_at_signature() {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), Wire::Direct);
        let mut o = opts(SkAlgorithm::Ed25519);
        o.user_verification = true;
        let m = backend.with(|a| a.generate(&o)).unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        let info = super::super::describe(&m.private_key, None).unwrap();
        assert!(
            info.flags
                .is_some_and(|f| f.user_verification && f.user_presence)
        );
        assert_eq!(
            sk_err(backend.with(|a| a.sign(&key, b"d", None)).unwrap_err()),
            Fido2Error::PinRequired
        );
        let blob = backend
            .with(|a| a.sign(&key, b"d", Some("123456")))
            .unwrap();
        // UV flag set in the signed flags byte.
        let sig = russh::keys::ssh_key::Signature::decode(&mut &blob[..]).unwrap();
        let flags = sig.as_bytes()[sig.as_bytes().len() - 5];
        assert_eq!(flags & FLAG_USER_VERIFICATION, FLAG_USER_VERIFICATION);
        assert_eq!(flags & FLAG_USER_PRESENCE, FLAG_USER_PRESENCE);
        Verifier::verify(key.public_key().key_data(), b"d", &sig).unwrap();
    }

    #[test]
    fn no_touch_key_skips_user_presence() {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), Wire::Direct);
        let mut o = opts(SkAlgorithm::EcdsaP256);
        o.user_presence = false;
        let m = backend.with(|a| a.generate(&o)).unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        assert_eq!(token.touches(), 1);
        backend.with(|a| a.sign(&key, b"d", None)).unwrap();
        assert_eq!(token.touches(), 1, "up=false must not ask for a touch");
    }

    #[test]
    fn touch_timeout_and_denial() {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), Wire::Hid);
        let m = backend
            .with(|a| a.generate(&opts(SkAlgorithm::Ed25519)))
            .unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        token.configure(|c| c.touch = Touch::Timeout);
        assert_eq!(
            sk_err(backend.with(|a| a.sign(&key, b"d", None)).unwrap_err()),
            Fido2Error::Timeout
        );
        token.configure(|c| c.touch = Touch::Deny);
        assert_eq!(
            sk_err(backend.with(|a| a.sign(&key, b"d", None)).unwrap_err()),
            Fido2Error::Denied
        );
    }

    #[test]
    fn wrong_token_has_no_credential() {
        let a = SoftToken::default();
        let b = SoftToken::default();
        let m = SoftBackend::new(a, Wire::Direct)
            .with(|x| x.generate(&opts(SkAlgorithm::Ed25519)))
            .unwrap();
        let key = load_private_key(&m.private_key, None).unwrap();
        assert_eq!(
            sk_err(
                SoftBackend::new(b, Wire::Nfc)
                    .with(|x| x.sign(&key, b"d", None))
                    .unwrap_err()
            ),
            Fido2Error::WrongDevice
        );
    }

    #[test]
    fn resident_keys_are_loaded_like_ssh_keygen_k() {
        let token = SoftToken::default();
        let backend = SoftBackend::new(token.clone(), Wire::Hid);
        let mut ed = opts(SkAlgorithm::Ed25519);
        ed.resident = true;
        ed.user = Some("alice".into());
        let mut ec = opts(SkAlgorithm::EcdsaP256);
        ec.resident = true;
        ec.application = Some("ssh:work".into());
        let non_resident = opts(SkAlgorithm::Ed25519);
        let ed_m = backend.with(|a| a.generate(&ed)).unwrap();
        let ec_m = backend.with(|a| a.generate(&ec)).unwrap();
        backend.with(|a| a.generate(&non_resident)).unwrap();
        assert_eq!(token.credential_count(), 3);

        let loaded = backend
            .with(|a| a.load_resident("123456", Some("pp")))
            .unwrap();
        assert_eq!(loaded.len(), 2, "only resident credentials come back");
        let mut algs: Vec<Algorithm> = loaded
            .iter()
            .map(|m| {
                load_private_key(&m.private_key, Some("pp"))
                    .unwrap()
                    .algorithm()
            })
            .collect();
        algs.sort_by_key(|a| a.as_str().to_string());
        assert_eq!(
            algs,
            vec![Algorithm::SkEcdsaSha2NistP256, Algorithm::SkEd25519]
        );
        for m in &loaded {
            let info = super::super::describe(&m.private_key, Some("pp")).unwrap();
            assert!(info.flags.is_some_and(|f| f.resident && f.user_presence));
            let key = load_private_key(&m.private_key, Some("pp")).unwrap();
            let handle = super::super::SkHandle::of(&key).unwrap();
            assert_eq!(handle.flags, FLAG_USER_PRESENCE | FLAG_RESIDENT);
            // Loaded handles sign with the same credential as the originals.
            let blob = backend.with(|a| a.sign(&key, b"d", None)).unwrap();
            let sig = russh::keys::ssh_key::Signature::decode(&mut &blob[..]).unwrap();
            Verifier::verify(key.public_key().key_data(), b"d", &sig).unwrap();
            let orig = if key.algorithm() == Algorithm::SkEd25519 {
                &ed_m
            } else {
                &ec_m
            };
            let orig_pub = orig.public_key.split_whitespace().nth(1).unwrap();
            assert_eq!(m.public_key.split_whitespace().nth(1).unwrap(), orig_pub);
        }
        assert_eq!(
            loaded
                .iter()
                .find(|m| m.public_key.starts_with("sk-ssh-ed25519"))
                .unwrap()
                .info
                .comment,
            "alice"
        );
        assert_eq!(
            sk_err(
                backend
                    .with(|a| a.load_resident("000000", None))
                    .unwrap_err()
            ),
            Fido2Error::PinInvalid { retries: Some(7) }
        );
    }

    #[test]
    fn ed25519_refused_by_p256_only_token() {
        let token = SoftToken::new(Config {
            ed25519: false,
            ..Config::default()
        });
        let backend = SoftBackend::new(token, Wire::Direct);
        assert!(matches!(
            sk_err(
                backend
                    .with(|a| a.generate(&opts(SkAlgorithm::Ed25519)))
                    .unwrap_err()
            ),
            Fido2Error::Unsupported(_)
        ));
        backend
            .with(|a| a.generate(&opts(SkAlgorithm::EcdsaP256)))
            .unwrap();
    }
}

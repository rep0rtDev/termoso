//! Browser bindings for `termoso-crypto`.
//!
//! Everything a Termoso web client needs to stay zero-knowledge runs here, in
//! Rust compiled to WebAssembly: OPAQUE, key derivation, key wrapping, sealed
//! boxes and entity encryption. JavaScript only moves opaque base64 strings
//! between this module and the HTTP API; it never implements crypto itself.
//!
//! Security boundary: values returned to JS (export key, account private key,
//! vault keys) live in JS memory for the duration of the session. The web
//! client must keep them in memory only (never `localStorage`) and drop them on
//! sign-out. Nothing in this crate logs or panics with secret material.

use serde::Serialize;
use termoso_crypto::aead::{self, Aad};
use termoso_crypto::encoding::{b64, unb64};
use termoso_crypto::kdf::{Label, derive_key};
use termoso_crypto::keys::{KeyPair, SymmetricKey, public_key_from_b64};
use termoso_crypto::opaque::{self, ClientLoginState, ClientRegistrationState};
use termoso_crypto::recovery::RecoveryKey;
use termoso_crypto::{CryptoError, sealed};
use wasm_bindgen::prelude::*;
use zeroize::Zeroizing;

/// Error surfaced to JavaScript as a plain `Error` with a short, secret-free message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmError(String);

impl WasmError {
    fn new(msg: &str) -> Self {
        Self(msg.to_owned())
    }
}

impl From<CryptoError> for WasmError {
    fn from(e: CryptoError) -> Self {
        Self(e.to_string())
    }
}

impl From<WasmError> for JsValue {
    fn from(e: WasmError) -> Self {
        js_sys::Error::new(&e.0).into()
    }
}

fn js_err(e: CryptoError) -> WasmError {
    e.into()
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, WasmError> {
    serde_wasm_bindgen::to_value(v).map_err(|e| WasmError(e.to_string()))
}

fn symmetric(b64_key: &str) -> Result<SymmetricKey, WasmError> {
    SymmetricKey::from_b64(b64_key).map_err(js_err)
}

fn keypair(private_b64: &str) -> Result<KeyPair, WasmError> {
    let secret = Zeroizing::new(unb64(private_b64).map_err(js_err)?);
    KeyPair::from_secret_bytes(&secret).map_err(js_err)
}

fn account_kek(export_key_b64: &str) -> Result<SymmetricKey, WasmError> {
    let ikm = Zeroizing::new(unb64(export_key_b64).map_err(js_err)?);
    derive_key(&ikm, Label::AccountKek).map_err(js_err)
}

/// Protocol version string embedded in every AAD label.
#[wasm_bindgen]
pub fn protocol_version() -> String {
    termoso_crypto::PROTOCOL_VERSION.to_string()
}

// ───────────────────────────── OPAQUE ─────────────────────────────

/// Result of the final OPAQUE registration step.
#[wasm_bindgen(getter_with_clone)]
pub struct RegistrationResult {
    /// `RegistrationUpload`, base64 — send to `/auth/register/finish`.
    pub upload: String,
    /// Export key, base64 — feed to [`create_account_keys`] / [`unlock_account`]. Never send.
    pub export_key: String,
}

/// OPAQUE registration (or password change) in progress. Create it, POST
/// `request` to the server, then call `finish` with the server's response.
#[wasm_bindgen]
pub struct OpaqueRegistration {
    state: Option<ClientRegistrationState>,
    request: String,
}

#[wasm_bindgen]
impl OpaqueRegistration {
    /// Step 1: derive the blinded registration request.
    #[wasm_bindgen(constructor)]
    pub fn new(password: &str) -> Result<OpaqueRegistration, WasmError> {
        let (request, state) =
            opaque::client_registration_start(password.as_bytes()).map_err(js_err)?;
        Ok(Self {
            state: Some(state),
            request,
        })
    }

    /// Base64 `RegistrationRequest` for the server.
    #[wasm_bindgen(getter)]
    pub fn request(&self) -> String {
        self.request.clone()
    }

    /// Step 3: consume the server's `RegistrationResponse`. `user_id` is the
    /// lower-cased email. Consumes the state: call once.
    pub fn finish(
        &mut self,
        password: &str,
        user_id: &str,
        response: &str,
    ) -> Result<RegistrationResult, WasmError> {
        let state = self
            .state
            .take()
            .ok_or_else(|| WasmError::new("registration already finished"))?;
        let out = opaque::client_registration_finish(state, password.as_bytes(), user_id, response)
            .map_err(js_err)?;
        Ok(RegistrationResult {
            upload: out.upload_b64,
            export_key: b64(&out.export_key),
        })
    }
}

/// Result of the final OPAQUE login step.
#[wasm_bindgen(getter_with_clone)]
pub struct LoginResult {
    /// `CredentialFinalization`, base64 — send to `/auth/login/finish`.
    pub finalization: String,
    /// Export key, base64 — same value as at registration for this password.
    pub export_key: String,
}

/// OPAQUE login in progress.
#[wasm_bindgen]
pub struct OpaqueLogin {
    state: Option<ClientLoginState>,
    request: String,
}

#[wasm_bindgen]
impl OpaqueLogin {
    /// Step 1.
    #[wasm_bindgen(constructor)]
    pub fn new(password: &str) -> Result<OpaqueLogin, WasmError> {
        let (request, state) = opaque::client_login_start(password.as_bytes()).map_err(js_err)?;
        Ok(Self {
            state: Some(state),
            request,
        })
    }

    /// Base64 `CredentialRequest` for the server.
    #[wasm_bindgen(getter)]
    pub fn request(&self) -> String {
        self.request.clone()
    }

    /// Step 3. Fails with "opaque protocol error" when the password is wrong —
    /// the server cannot tell, only the client can.
    pub fn finish(
        &mut self,
        password: &str,
        user_id: &str,
        response: &str,
    ) -> Result<LoginResult, WasmError> {
        let state = self
            .state
            .take()
            .ok_or_else(|| WasmError::new("login already finished"))?;
        let out = opaque::client_login_finish(state, password.as_bytes(), user_id, response)
            .map_err(js_err)?;
        Ok(LoginResult {
            finalization: out.finalization_b64,
            export_key: b64(&out.export_key),
        })
    }
}

// ───────────────────────────── account keys ─────────────────────────────

/// Everything produced when an account is created. The `*_upload` fields go to
/// the server verbatim; `private_key`, `personal_vault_key` and
/// `recovery_phrase` must stay on the client.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccountKeys {
    /// X25519 public key, base64.
    pub public_key: String,
    /// Private key wrapped with the account KEK.
    pub wrapped_private_key: String,
    /// Private key wrapped with the recovery KEK.
    pub recovery_wrapped_private_key: String,
    /// Recovery verifier, base64.
    pub recovery_verifier: String,
    /// Personal vault key sealed to `public_key`.
    pub personal_vault_sealed_key: String,
    /// 24-word recovery phrase — show once, never store.
    pub recovery_phrase: String,
    /// Account private key, base64 (session memory only).
    pub private_key: String,
    /// Personal vault key, base64 (session memory only).
    pub personal_vault_key: String,
}

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = r#"
export interface NewAccountKeys {
  publicKey: string;
  wrappedPrivateKey: string;
  recoveryWrappedPrivateKey: string;
  recoveryVerifier: string;
  personalVaultSealedKey: string;
  recoveryPhrase: string;
  privateKey: string;
  personalVaultKey: string;
}
export interface RecoveryRotation {
  recoveryPhrase: string;
  recoveryWrappedPrivateKey: string;
  recoveryVerifier: string;
}
export interface GeneratedKeyPair {
  privateKey: string;
  publicKey: string;
}
"#;

/// Generate a fresh account key set from the OPAQUE export key.
#[wasm_bindgen(unchecked_return_type = "NewAccountKeys")]
pub fn create_account_keys(export_key: &str) -> Result<JsValue, WasmError> {
    let kek = account_kek(export_key)?;
    let pair = KeyPair::generate();
    let recovery = RecoveryKey::generate();
    let recovery_kek = recovery.kek().map_err(js_err)?;
    let vault_key = SymmetricKey::generate();
    let secret = SymmetricKey::from_bytes(pair.secret_bytes());

    let keys = NewAccountKeys {
        public_key: pair.public_b64(),
        wrapped_private_key: aead::wrap_key(&kek, &Aad::account_private_key(), &secret)
            .map_err(js_err)?,
        recovery_wrapped_private_key: aead::wrap_key(
            &recovery_kek,
            &Aad::recovery_private_key(),
            &secret,
        )
        .map_err(js_err)?,
        recovery_verifier: recovery.verifier_b64().map_err(js_err)?,
        personal_vault_sealed_key: sealed::seal_vault_key(pair.public(), &vault_key)
            .map_err(js_err)?,
        recovery_phrase: recovery.phrase(),
        private_key: secret.to_b64(),
        personal_vault_key: vault_key.to_b64(),
    };
    to_js(&keys)
}

/// Unwrap the account private key after login. Verifies it matches
/// `public_key` so a tampered blob is rejected. Returns base64 private key.
#[wasm_bindgen]
pub fn unlock_account(
    export_key: &str,
    wrapped_private_key: &str,
    public_key: &str,
) -> Result<String, WasmError> {
    let kek = account_kek(export_key)?;
    let secret =
        aead::unwrap_key(&kek, &Aad::account_private_key(), wrapped_private_key).map_err(js_err)?;
    check_public(&secret, public_key)?;
    Ok(secret.to_b64())
}

/// Unwrap the account private key with the recovery phrase (password reset).
#[wasm_bindgen]
pub fn unlock_with_recovery(
    recovery_phrase: &str,
    recovery_wrapped_private_key: &str,
    public_key: &str,
) -> Result<String, WasmError> {
    let recovery = RecoveryKey::parse(recovery_phrase).map_err(js_err)?;
    let kek = recovery.kek().map_err(js_err)?;
    let secret = aead::unwrap_key(
        &kek,
        &Aad::recovery_private_key(),
        recovery_wrapped_private_key,
    )
    .map_err(js_err)?;
    check_public(&secret, public_key)?;
    Ok(secret.to_b64())
}

fn check_public(secret: &SymmetricKey, public_key: &str) -> Result<(), WasmError> {
    let pair = KeyPair::from_secret_bytes(secret.as_bytes()).map_err(js_err)?;
    if pair.public_b64() != public_key {
        return Err(WasmError::new("account key does not match public key"));
    }
    Ok(())
}

/// Re-wrap the private key with a KEK derived from a *new* export key
/// (password change / recovery). Returns the new `wrapped_private_key`.
#[wasm_bindgen]
pub fn rewrap_private_key(private_key: &str, new_export_key: &str) -> Result<String, WasmError> {
    let kek = account_kek(new_export_key)?;
    let secret = symmetric(private_key)?;
    aead::wrap_key(&kek, &Aad::account_private_key(), &secret).map_err(js_err)
}

/// A freshly generated recovery key bound to the account private key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryRotation {
    /// Show once.
    pub recovery_phrase: String,
    /// Private key wrapped with the new recovery KEK.
    pub recovery_wrapped_private_key: String,
    /// New verifier, base64.
    pub recovery_verifier: String,
}

/// Generate a new recovery phrase for an existing account.
#[wasm_bindgen(unchecked_return_type = "RecoveryRotation")]
pub fn rotate_recovery(private_key: &str) -> Result<JsValue, WasmError> {
    let secret = symmetric(private_key)?;
    let recovery = RecoveryKey::generate();
    let kek = recovery.kek().map_err(js_err)?;
    to_js(&RecoveryRotation {
        recovery_phrase: recovery.phrase(),
        recovery_wrapped_private_key: aead::wrap_key(&kek, &Aad::recovery_private_key(), &secret)
            .map_err(js_err)?,
        recovery_verifier: recovery.verifier_b64().map_err(js_err)?,
    })
}

/// Verifier for `/auth/recovery/start`, derived from a typed phrase.
#[wasm_bindgen]
pub fn recovery_verifier(recovery_phrase: &str) -> Result<String, WasmError> {
    RecoveryKey::parse(recovery_phrase)
        .and_then(|r| r.verifier_b64())
        .map_err(js_err)
}

/// Normalize a typed phrase into its 24 words; errors if invalid.
#[wasm_bindgen]
pub fn recovery_words(recovery_phrase: &str) -> Result<Vec<String>, WasmError> {
    let r = RecoveryKey::parse(recovery_phrase).map_err(js_err)?;
    Ok(r.words().into_iter().map(str::to_owned).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedKeyPair {
    /// X25519 secret, base64. Handed to a bridge or device once, never stored server-side.
    pub private_key: String,
    /// Matching public key, base64.
    pub public_key: String,
}

/// Fresh X25519 key pair for a recipient other than the account (API bridge).
#[wasm_bindgen(unchecked_return_type = "GeneratedKeyPair")]
pub fn generate_key_pair() -> Result<JsValue, WasmError> {
    let pair = KeyPair::generate();
    to_js(&GeneratedKeyPair {
        private_key: SymmetricKey::from_bytes(pair.secret_bytes()).to_b64(),
        public_key: pair.public_b64(),
    })
}

/// Public key (base64) for a base64 private key.
#[wasm_bindgen]
pub fn public_key_of(private_key: &str) -> Result<String, WasmError> {
    Ok(keypair(private_key)?.public_b64())
}

// ───────────────────────────── vault keys ─────────────────────────────

/// Fresh 256-bit vault key, base64.
#[wasm_bindgen]
pub fn generate_vault_key() -> String {
    SymmetricKey::generate().to_b64()
}

/// Seal a vault key to a member's public key.
#[wasm_bindgen]
pub fn seal_vault_key(recipient_public_key: &str, vault_key: &str) -> Result<String, WasmError> {
    let pk = public_key_from_b64(recipient_public_key).map_err(js_err)?;
    let key = symmetric(vault_key)?;
    sealed::seal_vault_key(&pk, &key).map_err(js_err)
}

/// Open a vault key sealed to us. Returns base64 key.
#[wasm_bindgen]
pub fn open_vault_key(private_key: &str, sealed_key: &str) -> Result<String, WasmError> {
    let pair = keypair(private_key)?;
    Ok(sealed::open_vault_key(&pair, sealed_key)
        .map_err(js_err)?
        .to_b64())
}

// ───────────────────────────── entity encryption ─────────────────────────────

/// Encrypt a UTF-8 string bound to `termoso/v1/entity/<kind>/<id>/<field>`.
#[wasm_bindgen]
pub fn encrypt_field(
    key: &str,
    kind: &str,
    entity_id: &str,
    field: &str,
    plaintext: &str,
) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    aead::encrypt_str(&k, &Aad::entity_field(kind, entity_id, field), plaintext).map_err(js_err)
}

/// Inverse of [`encrypt_field`].
#[wasm_bindgen]
pub fn decrypt_field(
    key: &str,
    kind: &str,
    entity_id: &str,
    field: &str,
    envelope: &str,
) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    aead::decrypt_str(&k, &Aad::entity_field(kind, entity_id, field), envelope).map_err(js_err)
}

/// Encrypt a whole entity payload (JSON string) bound to `termoso/v1/entity/<kind>/<id>`.
#[wasm_bindgen]
pub fn encrypt_entity(
    key: &str,
    kind: &str,
    entity_id: &str,
    json: &str,
) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    aead::encrypt_str(&k, &Aad::entity(kind, entity_id), json).map_err(js_err)
}

/// Inverse of [`encrypt_entity`].
#[wasm_bindgen]
pub fn decrypt_entity(
    key: &str,
    kind: &str,
    entity_id: &str,
    envelope: &str,
) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    aead::decrypt_str(&k, &Aad::entity(kind, entity_id), envelope).map_err(js_err)
}

/// Encrypt under an arbitrary label path (`termoso/v1/<parts...>`), e.g.
/// `["settings"]` for the account settings blob.
#[wasm_bindgen]
pub fn encrypt_labeled(
    key: &str,
    label: Vec<String>,
    plaintext: &str,
) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    let parts: Vec<&str> = label.iter().map(String::as_str).collect();
    aead::encrypt_str(&k, &Aad::label(&parts), plaintext).map_err(js_err)
}

/// Inverse of [`encrypt_labeled`].
#[wasm_bindgen]
pub fn decrypt_labeled(key: &str, label: Vec<String>, envelope: &str) -> Result<String, WasmError> {
    let k = symmetric(key)?;
    let parts: Vec<&str> = label.iter().map(String::as_str).collect();
    aead::decrypt_str(&k, &Aad::label(&parts), envelope).map_err(js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_roundtrip_native() {
        // Exercise the non-wasm parts of the pipeline with a fake export key.
        let export = b64(&[7u8; 64]);
        let kek = account_kek(&export).unwrap();
        let pair = KeyPair::generate();
        let secret = SymmetricKey::from_bytes(pair.secret_bytes());
        let wrapped = aead::wrap_key(&kek, &Aad::account_private_key(), &secret).unwrap();
        let unwrapped = unlock_account(&export, &wrapped, &pair.public_b64()).unwrap();
        assert_eq!(unwrapped, secret.to_b64());
        assert!(unlock_account(&export, &wrapped, &KeyPair::generate().public_b64()).is_err());

        let vk = generate_vault_key();
        let sealed = seal_vault_key(&pair.public_b64(), &vk).unwrap();
        assert_eq!(open_vault_key(&unwrapped, &sealed).unwrap(), vk);

        let ct = encrypt_field(&vk, "host", "id", "address", "10.0.0.1").unwrap();
        assert_eq!(
            decrypt_field(&vk, "host", "id", "address", &ct).unwrap(),
            "10.0.0.1"
        );
        assert!(decrypt_field(&vk, "host", "id", "label", &ct).is_err());
    }

    #[test]
    fn recovery_roundtrip_native() {
        let pair = KeyPair::generate();
        let secret = SymmetricKey::from_bytes(pair.secret_bytes());
        let recovery = RecoveryKey::generate();
        let wrapped = aead::wrap_key(
            &recovery.kek().unwrap(),
            &Aad::recovery_private_key(),
            &secret,
        )
        .unwrap();
        let phrase = recovery.phrase();
        assert_eq!(recovery_words(&phrase).unwrap().len(), 24);
        assert_eq!(
            recovery_verifier(&phrase).unwrap(),
            recovery.verifier_b64().unwrap()
        );
        let got =
            unlock_with_recovery(&phrase.to_uppercase(), &wrapped, &pair.public_b64()).unwrap();
        assert_eq!(got, secret.to_b64());
        assert!(recovery_verifier("not a phrase").is_err());
    }
}

//! OPAQUE (RFC 9807 draft) password-authenticated key exchange.
//!
//! Cipher suite: ristretto255 OPRF, 3DH over ristretto255 with SHA-512,
//! Argon2id as the key-stretching function.
//!
//! The password never leaves the client, and the server's registration record
//! is not vulnerable to pre-computation attacks (unlike SRP verifiers).
//! After a successful login both parties share `session_key`; the client also
//! obtains `export_key`, a stable per-user secret used to derive the account
//! KEK (see [`crate::kdf`]).

use argon2::{Algorithm, Argon2, Params, Version};
use opaque_ke::ciphersuite::CipherSuite;
use opaque_ke::rand::rngs::OsRng;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, CredentialFinalization, CredentialRequest,
    CredentialResponse, Identifiers, RegistrationRequest, RegistrationResponse, RegistrationUpload,
    ServerLogin, ServerLoginParameters, ServerRegistration, ServerSetup,
};
use zeroize::Zeroizing;

use crate::CryptoError;
use crate::encoding::{b64, unb64};

/// The Termoso OPAQUE cipher suite.
pub struct Suite;

impl CipherSuite for Suite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::TripleDh<opaque_ke::Ristretto255, sha2::Sha512>;
    type Ksf = Argon2<'static>;
}

/// Server identifier mixed into the key exchange. Clients and servers must agree.
pub const SERVER_ID: &[u8] = b"termoso";

/// Argon2id parameters for the OPAQUE KSF (64 MiB, 3 passes, 1 lane).
/// These run on the client only.
pub fn ksf() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 3, 1, None).expect("valid argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

fn oe<E: std::fmt::Debug>(e: E) -> CryptoError {
    CryptoError::Opaque(format!("{e:?}"))
}

fn ids(user: &str) -> Identifiers<'_> {
    Identifiers {
        client: Some(user.as_bytes()),
        server: Some(SERVER_ID),
    }
}

// ───────────────────────────── server ─────────────────────────────

/// Long-lived server keying material (OPRF seed + server key pair).
/// Generated once per deployment and stored encrypted at rest.
pub struct Server {
    setup: ServerSetup<Suite>,
}

impl Server {
    /// Generate fresh server setup.
    pub fn generate() -> Self {
        Self {
            setup: ServerSetup::<Suite>::new(&mut OsRng),
        }
    }

    /// Serialize to bytes for storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.setup.serialize().to_vec()
    }

    /// Restore from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self {
            setup: ServerSetup::<Suite>::deserialize(bytes).map_err(oe)?,
        })
    }

    /// Registration step 2: answer the client's registration request.
    pub fn registration_start(
        &self,
        user_id: &str,
        request_b64: &str,
    ) -> Result<String, CryptoError> {
        let req = RegistrationRequest::<Suite>::deserialize(&unb64(request_b64)?).map_err(oe)?;
        let res =
            ServerRegistration::<Suite>::start(&self.setup, req, user_id.as_bytes()).map_err(oe)?;
        Ok(b64(&res.message.serialize()))
    }

    /// Registration step 4: turn the client's upload into the record to store.
    pub fn registration_finish(upload_b64: &str) -> Result<Vec<u8>, CryptoError> {
        let upload = RegistrationUpload::<Suite>::deserialize(&unb64(upload_b64)?).map_err(oe)?;
        Ok(ServerRegistration::<Suite>::finish(upload)
            .serialize()
            .to_vec())
    }

    /// Login step 2. `record` is `None` for unknown users (a fake response is
    /// produced so user enumeration is not possible). Returns
    /// `(credential_response_b64, server_state_bytes)`; keep the state for
    /// [`Server::login_finish`].
    pub fn login_start(
        &self,
        user_id: &str,
        record: Option<&[u8]>,
        request_b64: &str,
    ) -> Result<(String, Vec<u8>), CryptoError> {
        let record = match record {
            Some(r) => Some(ServerRegistration::<Suite>::deserialize(r).map_err(oe)?),
            None => None,
        };
        let req = CredentialRequest::<Suite>::deserialize(&unb64(request_b64)?).map_err(oe)?;
        let res = ServerLogin::start(
            &mut OsRng,
            &self.setup,
            record,
            req,
            user_id.as_bytes(),
            ServerLoginParameters {
                context: None,
                identifiers: ids(user_id),
            },
        )
        .map_err(oe)?;
        Ok((
            b64(&res.message.serialize()),
            res.state.serialize().to_vec(),
        ))
    }

    /// Login step 4: verify the client's finalization. Returns the session key.
    pub fn login_finish(
        user_id: &str,
        state: &[u8],
        finalization_b64: &str,
    ) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let state = ServerLogin::<Suite>::deserialize(state).map_err(oe)?;
        let fin =
            CredentialFinalization::<Suite>::deserialize(&unb64(finalization_b64)?).map_err(oe)?;
        let res = state
            .finish(
                fin,
                ServerLoginParameters {
                    context: None,
                    identifiers: ids(user_id),
                },
            )
            .map_err(oe)?;
        Ok(Zeroizing::new(res.session_key.to_vec()))
    }
}

// ───────────────────────────── client ─────────────────────────────

/// Client-side registration state between step 1 and step 3.
pub struct ClientRegistrationState {
    state: ClientRegistration<Suite>,
}

/// Result of finishing registration on the client.
pub struct ClientRegistrationOutput {
    /// Base64 `RegistrationUpload` to send to the server.
    pub upload_b64: String,
    /// 64-byte export key — derive the account KEK from it. Never send it.
    pub export_key: Zeroizing<Vec<u8>>,
}

/// Registration step 1. Returns `(request_b64, state)`.
pub fn client_registration_start(
    password: &[u8],
) -> Result<(String, ClientRegistrationState), CryptoError> {
    let res = ClientRegistration::<Suite>::start(&mut OsRng, password).map_err(oe)?;
    Ok((
        b64(&res.message.serialize()),
        ClientRegistrationState { state: res.state },
    ))
}

/// Registration step 3.
pub fn client_registration_finish(
    state: ClientRegistrationState,
    password: &[u8],
    user_id: &str,
    response_b64: &str,
) -> Result<ClientRegistrationOutput, CryptoError> {
    let resp = RegistrationResponse::<Suite>::deserialize(&unb64(response_b64)?).map_err(oe)?;
    let ksf = ksf();
    let res = state
        .state
        .finish(
            &mut OsRng,
            password,
            resp,
            ClientRegistrationFinishParameters::new(ids(user_id), Some(&ksf)),
        )
        .map_err(oe)?;
    Ok(ClientRegistrationOutput {
        upload_b64: b64(&res.message.serialize()),
        export_key: Zeroizing::new(res.export_key.to_vec()),
    })
}

/// Client-side login state between step 1 and step 3.
pub struct ClientLoginState {
    state: ClientLogin<Suite>,
}

impl ClientLoginState {
    /// Serialize (e.g. to hold across an async boundary in a foreign runtime).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.state.serialize().to_vec()
    }

    /// Restore.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        Ok(Self {
            state: ClientLogin::<Suite>::deserialize(bytes).map_err(oe)?,
        })
    }
}

/// Result of finishing login on the client.
pub struct ClientLoginOutput {
    /// Base64 `CredentialFinalization` to send to the server.
    pub finalization_b64: String,
    /// Shared session key (matches the server's).
    pub session_key: Zeroizing<Vec<u8>>,
    /// 64-byte export key — same value as at registration for the same password.
    pub export_key: Zeroizing<Vec<u8>>,
}

/// Login step 1. Returns `(request_b64, state)`.
pub fn client_login_start(password: &[u8]) -> Result<(String, ClientLoginState), CryptoError> {
    let res = ClientLogin::<Suite>::start(&mut OsRng, password).map_err(oe)?;
    Ok((
        b64(&res.message.serialize()),
        ClientLoginState { state: res.state },
    ))
}

/// Login step 3. Fails if the password is wrong (the server learns nothing).
pub fn client_login_finish(
    state: ClientLoginState,
    password: &[u8],
    user_id: &str,
    response_b64: &str,
) -> Result<ClientLoginOutput, CryptoError> {
    let resp = CredentialResponse::<Suite>::deserialize(&unb64(response_b64)?).map_err(oe)?;
    let ksf = ksf();
    let res = state
        .state
        .finish(
            &mut OsRng,
            password,
            resp,
            ClientLoginFinishParameters::new(None, ids(user_id), Some(&ksf)),
        )
        .map_err(|_| CryptoError::Opaque("invalid credentials".into()))?;
    Ok(ClientLoginOutput {
        finalization_b64: b64(&res.message.serialize()),
        session_key: Zeroizing::new(res.session_key.to_vec()),
        export_key: Zeroizing::new(res.export_key.to_vec()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register(server: &Server, user: &str, pw: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let (req, st) = client_registration_start(pw).unwrap();
        let resp = server.registration_start(user, &req).unwrap();
        let out = client_registration_finish(st, pw, user, &resp).unwrap();
        let record = Server::registration_finish(&out.upload_b64).unwrap();
        (record, out.export_key.to_vec())
    }

    #[test]
    fn full_flow_and_export_key_stability() {
        let server = Server::generate();
        let server = Server::from_bytes(&server.to_bytes()).unwrap();
        let (record, export_at_reg) = register(&server, "alice@example.com", b"correct horse");

        let (req, st) = client_login_start(b"correct horse").unwrap();
        let (resp, sstate) = server
            .login_start("alice@example.com", Some(&record), &req)
            .unwrap();
        let out = client_login_finish(st, b"correct horse", "alice@example.com", &resp).unwrap();
        let sk = Server::login_finish("alice@example.com", &sstate, &out.finalization_b64).unwrap();
        assert_eq!(sk.as_slice(), out.session_key.as_slice());
        assert_eq!(out.export_key.as_slice(), export_at_reg.as_slice());
    }

    #[test]
    fn wrong_password_fails_on_client() {
        let server = Server::generate();
        let (record, _) = register(&server, "bob", b"pw1");
        let (req, st) = client_login_start(b"pw2").unwrap();
        let (resp, _) = server.login_start("bob", Some(&record), &req).unwrap();
        assert!(client_login_finish(st, b"pw2", "bob", &resp).is_err());
    }

    #[test]
    fn unknown_user_gets_fake_response() {
        let server = Server::generate();
        let (req, st) = client_login_start(b"pw").unwrap();
        let (resp, _) = server.login_start("ghost", None, &req).unwrap();
        assert!(client_login_finish(st, b"pw", "ghost", &resp).is_err());
    }
}

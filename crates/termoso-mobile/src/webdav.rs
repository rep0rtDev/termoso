//! WebDAV shares opened from the file browser. Credentials come from the
//! host's WebDAV section (an encrypted identity); a missing password and an
//! untrusted certificate go through the same prompt broker the SSH path
//! uses, so Kotlin handles them like a password / host-key question.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use termoso_core::error::CoreError;
use termoso_core::model::{Identity, ResolvedHost};
use termoso_core::store::Store;
use termoso_core::webdav::{self, TlsPolicy, WebDav, WebDavConfig};
use zeroize::Zeroizing;

use crate::connect::{ConnectStage, Connector, HostKeyChoice, PromptAnswer, PromptRequest};
use crate::error::{MobileError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_PASSWORD_ATTEMPTS: u32 = 3;

/// `host/path` form of a share URL for titles: no scheme, no trailing slash.
pub(crate) fn display_of(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.trim_end_matches('/').to_string()
}

/// Open the WebDAV section of `resolved`, asking through `conn` for a
/// password or a certificate decision when the saved ones are not enough.
pub(crate) async fn connect_webdav(
    conn: &Arc<Connector>,
    resolved: &ResolvedHost,
    spool_dir: PathBuf,
) -> Result<WebDav> {
    let Some(cfg) = resolved.webdav.clone() else {
        return Err(MobileError::invalid("this host has no WebDAV section"));
    };
    let url = webdav::normalize_url(&cfg.url)?.to_string();
    let identity = resolved.webdav_identity.as_ref().map(|i| &i.data);
    let base = WebDavConfig::default().with_identity(identity)?;
    let username = base.username.clone();
    let mut password = base.password.clone().map(Zeroizing::new);
    let mut tls = cfg
        .certificate_fingerprint
        .clone()
        .map(TlsPolicy::Pinned)
        .unwrap_or_default();
    let mut attempts = 0;
    let mut asked_password = false;

    conn.ui.phase(
        ConnectStage::ConnectingTo {
            target: display_of(&url),
        },
        None,
    );
    loop {
        let attempt = WebDavConfig {
            url: url.clone(),
            username: username.clone(),
            password: password.as_ref().map(|p| p.to_string()),
            tls: tls.clone(),
            connect_timeout: CONNECT_TIMEOUT,
            spool_dir: Some(spool_dir.clone()),
            ..base.clone()
        };
        match WebDav::connect(attempt).await {
            Ok(dav) => return Ok(dav),
            Err(CoreError::CertificateRejected { host, fingerprint })
                if matches!(tls, TlsPolicy::System) =>
            {
                let answer = conn
                    .ask(PromptRequest::Certificate {
                        host,
                        fingerprint: fingerprint.clone(),
                    })
                    .await;
                match answer {
                    Some(PromptAnswer::HostKey {
                        decision: HostKeyChoice::AcceptOnce,
                    }) => tls = TlsPolicy::Pinned(fingerprint),
                    Some(PromptAnswer::HostKey {
                        decision: HostKeyChoice::AcceptAndSave,
                    }) => {
                        pin_certificate(&conn.store, resolved, &fingerprint);
                        tls = TlsPolicy::Pinned(fingerprint);
                    }
                    _ => {
                        conn.give_up();
                        return Err(MobileError::Cancelled);
                    }
                }
            }
            Err(CoreError::AuthFailed { .. })
                if username.is_some()
                    && base.bearer_token.is_none()
                    && attempts < MAX_PASSWORD_ATTEMPTS =>
            {
                attempts += 1;
                let answer = conn
                    .ask(PromptRequest::Password {
                        username: username.clone().unwrap_or_default(),
                        retry: asked_password || password.is_some(),
                    })
                    .await;
                asked_password = true;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        let value = Zeroizing::new(value);
                        if remember {
                            remember_password(&conn.store, resolved, &value);
                        }
                        password = Some(value);
                    }
                    _ => {
                        conn.give_up();
                        return Err(MobileError::Cancelled);
                    }
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Persist an accepted certificate on the host's WebDAV section.
fn pin_certificate(store: &Store, resolved: &ResolvedHost, fingerprint: &str) {
    let Some(cfg_id) = resolved.host.data.webdav_config_id else {
        return;
    };
    let Some(mut cfg) = resolved.webdav.clone() else {
        return;
    };
    cfg.certificate_fingerprint = Some(fingerprint.to_string());
    if let Err(e) = store.update(cfg_id, &cfg) {
        tracing::warn!("pin WebDAV certificate: {e}");
    }
}

/// Store a password the user asked to remember on the section's identity
/// (the username lives there, so a prompt implies the identity exists).
fn remember_password(store: &Store, resolved: &ResolvedHost, value: &Zeroizing<String>) {
    let Some(ident) = &resolved.webdav_identity else {
        return;
    };
    let data = Identity {
        password: Some(value.to_string()),
        ..ident.data.clone()
    };
    if let Err(e) = store.update(ident.id, &data) {
        tracing::warn!("remember WebDAV password: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_drops_scheme_and_trailing_slash() {
        assert_eq!(
            display_of("https://cloud.example.com/remote.php/dav/"),
            "cloud.example.com/remote.php/dav"
        );
        assert_eq!(display_of("http://nas.local:8080/"), "nas.local:8080");
    }
}

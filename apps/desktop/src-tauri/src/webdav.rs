//! WebDAV shares opened from the file browser. Credentials come from the
//! host's WebDAV section (an encrypted identity); a missing password and an
//! untrusted certificate are put to the user through the same prompt broker
//! the SSH path uses, so the UI handles them like a password / host-key
//! question.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use termoso_core::error::CoreError;
use termoso_core::hostkey::HostKeyDecision;
use termoso_core::model::ResolvedHost;
use termoso_core::remote::RemoteFs;
use termoso_core::webdav::{self, TlsPolicy, WebDav, WebDavConfig};
use termoso_proto::entities::payload::Identity;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::prompts::{PromptAnswer, PromptRequest};
use crate::state::AppState;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// An open share.
pub struct Connection {
    pub label: String,
    /// `host/path` shown in the tab.
    pub display: String,
    pub fs: Arc<dyn RemoteFs>,
}

fn spool_dir() -> PathBuf {
    std::env::temp_dir().join("termoso-webdav")
}

/// Human-readable target for prompts and tab titles: the URL without scheme
/// or trailing slash.
fn display_of(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped.trim_end_matches('/').to_string()
}

/// Open the WebDAV section of `host_id`, asking the user for a password or
/// a certificate decision when needed. `session_id` scopes the prompts.
pub async fn connect<R: Runtime>(
    app: &AppHandle<R>,
    session_id: Uuid,
    host_id: Uuid,
) -> Result<Connection> {
    let state = app.state::<AppState>();
    let resolved = state.store.resolve_host(host_id)?;
    let Some(cfg) = resolved.webdav.clone() else {
        return Err(DesktopError::invalid("this host has no WebDAV section"));
    };
    let url = webdav::normalize_url(&cfg.url)?.to_string();
    let display = display_of(&url);
    let identity = resolved.webdav_identity.as_ref().map(|i| &i.data);
    let base = WebDavConfig::default().with_identity(identity)?;
    let username = base.username.clone();
    let mut password = base.password.clone().map(Zeroizing::new);
    let mut tls = cfg
        .certificate_fingerprint
        .clone()
        .map(TlsPolicy::Pinned)
        .unwrap_or_default();
    let mut asked_password = false;

    loop {
        let attempt = WebDavConfig {
            url: url.clone(),
            username: username.clone(),
            password: password.as_ref().map(|p| p.to_string()),
            tls: tls.clone(),
            connect_timeout: CONNECT_TIMEOUT,
            spool_dir: Some(spool_dir()),
            ..base.clone()
        };
        match WebDav::connect(attempt).await {
            Ok(dav) => {
                return Ok(Connection {
                    label: resolved.host.data.label.clone(),
                    display,
                    fs: Arc::new(dav),
                });
            }
            Err(CoreError::CertificateRejected { host, fingerprint })
                if matches!(tls, TlsPolicy::System) =>
            {
                let answer = state
                    .prompts
                    .ask(
                        app,
                        session_id,
                        display.clone(),
                        PromptRequest::Certificate {
                            host,
                            fingerprint: fingerprint.clone(),
                        },
                    )
                    .await;
                match answer {
                    Some(PromptAnswer::HostKey {
                        decision: HostKeyDecision::AcceptOnce,
                    }) => tls = TlsPolicy::Pinned(fingerprint),
                    Some(PromptAnswer::HostKey {
                        decision: HostKeyDecision::AcceptAndSave,
                    }) => {
                        pin_certificate(&state, &resolved, &fingerprint);
                        tls = TlsPolicy::Pinned(fingerprint);
                    }
                    _ => return Err(CoreError::Cancelled.into()),
                }
            }
            Err(CoreError::AuthFailed { .. })
                if username.is_some() && base.bearer_token.is_none() =>
            {
                let answer = state
                    .prompts
                    .ask(
                        app,
                        session_id,
                        display.clone(),
                        PromptRequest::Password {
                            username: username.clone().unwrap_or_default(),
                            retry: asked_password,
                        },
                    )
                    .await;
                match answer {
                    Some(PromptAnswer::Secret { value, remember }) => {
                        if remember {
                            remember_password(&state, &resolved, &value);
                        }
                        password = Some(value);
                        asked_password = true;
                    }
                    _ => return Err(CoreError::Cancelled.into()),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Persist an accepted certificate on the host's WebDAV section.
fn pin_certificate(state: &AppState, resolved: &ResolvedHost, fingerprint: &str) {
    let Some(cfg_id) = resolved.host.data.webdav_config_id else {
        return;
    };
    let Some(mut cfg) = resolved.webdav.clone() else {
        return;
    };
    cfg.certificate_fingerprint = Some(fingerprint.to_string());
    if let Err(e) = state.store.update(cfg_id, &cfg) {
        tracing::warn!(error = %e, "could not pin WebDAV certificate");
    }
}

/// Store a password the user asked to remember on the section's identity
/// (the username lives there, so a prompt implies the identity exists).
fn remember_password(state: &AppState, resolved: &ResolvedHost, value: &Zeroizing<String>) {
    let Some(ident) = &resolved.webdav_identity else {
        return;
    };
    let data = Identity {
        password: Some(value.to_string()),
        ..ident.data.clone()
    };
    if let Err(e) = state.store.update(ident.id, &data) {
        tracing::warn!(error = %e, "could not remember WebDAV password");
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

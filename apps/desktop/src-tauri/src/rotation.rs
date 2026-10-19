//! Client side of key rotation: the server only knows *that* a secret is
//! due; generating the new key, installing it on the hosts and replacing
//! it in the vault happens here, on a device that holds the vault key.

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use termoso_client::{hosts, keychain};
use termoso_core::keys::{self, KeyAlgorithm};
use termoso_core::model::{Identity, SshKey};
use termoso_proto::team::{RotationJob, UpdateRotationJobRequest};
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::sessions;
use crate::state::AppState;
use crate::team;

/// What one rotation run did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotationRunResult {
    pub job_id: Uuid,
    /// `done` or `failed` as reported to the server.
    pub status: String,
    /// Hosts where the new key is installed and the old one removed.
    pub hosts_updated: Vec<String>,
    /// `(host, reason)` for hosts that could not be updated; the old key is kept then.
    pub hosts_failed: Vec<(String, String)>,
}

fn algorithm_of(key_type: &str, bits: usize) -> Result<KeyAlgorithm> {
    Ok(match key_type {
        "ssh-ed25519" => KeyAlgorithm::Ed25519,
        "ssh-rsa" => KeyAlgorithm::Rsa {
            bits: if (2048..=8192).contains(&bits) && bits % 8 == 0 {
                bits
            } else {
                3072
            },
        },
        "ecdsa-sha2-nistp256" => KeyAlgorithm::EcdsaP256,
        "ecdsa-sha2-nistp384" => KeyAlgorithm::EcdsaP384,
        "ecdsa-sha2-nistp521" => KeyAlgorithm::EcdsaP521,
        other => {
            return Err(DesktopError::invalid(format!(
                "{other} keys cannot be regenerated here (security key or certificate); rotate it by hand and mark the job done"
            )));
        }
    })
}

/// Hosts of `vault_id` whose effective SSH login uses `key_id` — directly,
/// through an identity, or inherited from a group.
fn hosts_using_key(state: &AppState, vault_id: Uuid, key_id: Uuid) -> Result<Vec<(Uuid, String)>> {
    let store = state.store()?;
    let key_of_identity = |id: Option<Uuid>| -> Result<Option<Uuid>> {
        Ok(match id {
            Some(i) => store.get::<Identity>(i)?.and_then(|e| e.data.ssh_key_id),
            None => None,
        })
    };
    let mut out = Vec::new();
    for card in hosts::cards(&store, Some(vault_id))? {
        let form = hosts::form(&store, card.id)?;
        let mut k = form.ssh_key_id;
        if k.is_none() {
            k = key_of_identity(form.identity_id)?;
        }
        if k.is_none() {
            let inh = hosts::inherited(&store, form.group_id)?;
            k = inh.ssh_key_id;
            if k.is_none() {
                k = key_of_identity(inh.identity_id)?;
            }
        }
        if k == Some(key_id) {
            out.push((card.id, card.label.clone()));
        }
    }
    Ok(out)
}

async fn rotate_key<R: Runtime>(
    app: &AppHandle<R>,
    key_id: Uuid,
    vault_id: Uuid,
) -> Result<(Vec<String>, Vec<(String, String)>)> {
    let state = app.state::<AppState>();
    let (old, old_public) = {
        let store = state.store()?;
        let old = store.require::<SshKey>(key_id)?;
        if old.vault_id != vault_id {
            return Err(DesktopError::invalid("the key is not in the job's vault"));
        }
        let public = keychain::public_key(&store, key_id)?;
        (old, public)
    };
    let bits = keys::inspect(&old.data.private_key)
        .map(|i| i.bits)
        .unwrap_or(0);
    let algo = algorithm_of(&old.data.key_type, bits)?;
    let comment = old
        .data
        .public_key
        .as_deref()
        .and_then(|p| p.split_whitespace().nth(2))
        .unwrap_or(&old.data.label)
        .to_string();
    let fresh = keys::generate(algo, &comment, old.data.passphrase.as_deref())?;
    let targets = hosts_using_key(&state, vault_id, key_id)?;

    let mut updated = Vec::new();
    let mut failed = Vec::new();
    for (host_id, label) in targets {
        let conn = match sessions::connect_host(app, Uuid::new_v4(), host_id, Some(vault_id)).await
        {
            Ok(c) => c,
            Err(e) => {
                failed.push((label, e.to_string()));
                continue;
            }
        };
        let added = conn
            .client
            .exec(
                keychain::EXPORT_COMMAND,
                Some(bytes::Bytes::from(format!("{}\n", fresh.public_key))),
            )
            .await;
        let added: std::result::Result<(), String> = match added {
            Ok(out) => keychain::export_outcome(out.exit_code, &out.stdout, &out.stderr)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        if let Err(e) = added {
            conn.close().await;
            failed.push((label, format!("installing the new key: {e}")));
            continue;
        }
        let removed = conn
            .client
            .exec(
                keychain::REMOVE_COMMAND,
                Some(bytes::Bytes::from(format!("{old_public}\n"))),
            )
            .await;
        conn.close().await;
        match removed {
            Ok(out) if out.exit_code == Some(0) => updated.push(label),
            Ok(out) => failed.push((
                label,
                format!(
                    "new key installed, old one not removed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            )),
            Err(e) => failed.push((
                label,
                format!("new key installed, old one not removed: {e}"),
            )),
        }
    }
    if !failed.is_empty() {
        // Some hosts still trust only the old key: keep it in the vault so nobody is locked out.
        return Ok((updated, failed));
    }
    let mut data = old.data.clone();
    data.private_key = fresh.private_key.to_string();
    data.public_key = Some(fresh.public_key.clone());
    data.key_type = fresh.info.key_type.clone();
    state.store()?.update(key_id, &data)?;
    Ok((updated, failed))
}

fn update(status: &str, error: Option<String>) -> UpdateRotationJobRequest {
    UpdateRotationJobRequest {
        status: status.into(),
        error,
    }
}

/// Claim a queued job and perform it. Only SSH keys rotate automatically;
/// other kinds are reported back as a clear error without touching the job.
pub async fn run_job<R: Runtime>(app: &AppHandle<R>, job_id: Uuid) -> Result<RotationRunResult> {
    let job = team::rotation_jobs(app)
        .await?
        .into_iter()
        .find(|j| j.id == job_id)
        .ok_or_else(|| {
            DesktopError::invalid("this job is not available to this device (finished, cancelled or no write access to its vault)")
        })?;
    if job.entity_kind != "key" {
        return Err(DesktopError::invalid(
            "only SSH keys rotate automatically; replace this secret by hand and mark the job done",
        ));
    }
    team::rotation_update_job(app, job_id, update("running", None)).await?;
    match rotate_key(app, job.entity_id, job.vault_id).await {
        Ok((hosts_updated, hosts_failed)) if hosts_failed.is_empty() => {
            team::rotation_update_job(app, job_id, update("done", None)).await?;
            Ok(RotationRunResult {
                job_id,
                status: "done".into(),
                hosts_updated,
                hosts_failed,
            })
        }
        Ok((hosts_updated, hosts_failed)) => {
            let reason = hosts_failed
                .iter()
                .map(|(h, e)| format!("{h}: {e}"))
                .collect::<Vec<_>>()
                .join("; ");
            team::rotation_update_job(app, job_id, update("failed", Some(reason))).await?;
            Ok(RotationRunResult {
                job_id,
                status: "failed".into(),
                hosts_updated,
                hosts_failed,
            })
        }
        Err(e) => {
            let _ =
                team::rotation_update_job(app, job_id, update("failed", Some(e.to_string()))).await;
            Err(e)
        }
    }
}

/// The secret was replaced by hand (host password, certificate, hardware key).
pub async fn mark_done<R: Runtime>(app: &AppHandle<R>, job_id: Uuid) -> Result<RotationJob> {
    team::rotation_update_job(app, job_id, update("done", None)).await
}

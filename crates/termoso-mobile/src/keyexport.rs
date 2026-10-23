//! `ssh-copy-id` for the mobile apps: connect to a saved host with its
//! current credentials, append a keychain key's public half to
//! `~/.ssh/authorized_keys` there and hang up. Prompts (password, host key)
//! go to the listener exactly like a tunnel's; the result arrives on
//! `on_done` / `on_failed`.

use std::sync::{Arc, Mutex};

use termoso_client::keychain;
use termoso_core::ssh::SshTarget;
use termoso_core::store::Store;
use uuid::Uuid;

use crate::connect::{
    ConnectStage, ConnectUi, Connector, PromptAnswer, PromptRequest, SecretCache, connect_resolved,
    stage_label,
};
use crate::error::{MobileError, Result};
use crate::settings::MobileSettings;

/// What happened on the host.
#[derive(Debug, Clone, uniffi::Record)]
pub struct KeyExportResult {
    /// `false` when the key was already authorized there.
    pub added: bool,
    pub host_label: String,
    /// `user@host:port` the key was installed for.
    pub target: String,
}

#[uniffi::export(with_foreign)]
pub trait KeyExportListener: Send + Sync {
    /// Connection progress, human readable (`Connecting…`, `Authenticating…`).
    fn on_stage(&self, detail: String);
    fn on_prompt(&self, prompt_id: u64, request: PromptRequest);
    fn on_done(&self, result: KeyExportResult);
    fn on_failed(&self, message: String);
}

struct ExportUi {
    listener: Arc<dyn KeyExportListener>,
}

impl ConnectUi for ExportUi {
    fn phase(&self, stage: ConnectStage, hop: Option<String>) {
        self.listener.on_stage(stage_label(&stage, hop.as_deref()));
    }

    fn prompt(&self, prompt_id: u64, request: PromptRequest) {
        self.listener.on_prompt(prompt_id, request);
    }
}

pub(crate) struct KeyExportLaunch {
    pub store: Arc<Store>,
    pub secrets: Arc<SecretCache>,
    pub settings: MobileSettings,
    pub key_id: Uuid,
    pub host_id: Uuid,
    pub vault_id: Option<Uuid>,
    pub listener: Arc<dyn KeyExportListener>,
}

/// One in-flight export; keep it alive until `on_done` / `on_failed`.
#[derive(uniffi::Object)]
pub struct KeyExport {
    conn: Arc<Connector>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl KeyExport {
    pub(crate) fn launch(runtime: tokio::runtime::Handle, launch: KeyExportLaunch) -> Arc<Self> {
        let KeyExportLaunch {
            store,
            secrets,
            settings,
            key_id,
            host_id,
            vault_id,
            listener,
        } = launch;
        let conn = Arc::new(Connector::new(
            store.clone(),
            secrets,
            Arc::new(ExportUi {
                listener: listener.clone(),
            }),
        ));
        let job_conn = conn.clone();
        let task = runtime.spawn(async move {
            match run(&job_conn, &store, &settings, key_id, host_id, vault_id).await {
                Ok(r) => listener.on_done(r),
                Err(e) => listener.on_failed(job_conn.map_error(e).to_string()),
            }
        });
        Arc::new(Self {
            conn,
            task: Mutex::new(Some(task)),
        })
    }
}

#[uniffi::export]
impl KeyExport {
    pub fn answer(&self, prompt_id: u64, answer: PromptAnswer) -> bool {
        self.conn.answer(prompt_id, answer)
    }

    /// Abort: pending prompts are dropped and the connection attempt is
    /// cancelled. Neither `on_done` nor `on_failed` follows.
    pub fn cancel(&self) {
        self.conn.cancel_prompts();
        if let Some(t) = self.task.lock().expect("task poisoned").take() {
            t.abort();
        }
    }
}

async fn run(
    conn: &Arc<Connector>,
    store: &Store,
    settings: &MobileSettings,
    key_id: Uuid,
    host_id: Uuid,
    vault_id: Option<Uuid>,
) -> Result<KeyExportResult> {
    let public = keychain::public_key(store, key_id)?;
    let resolved = store.resolve_host_in(host_id, vault_id)?;
    if resolved.protocol() != "ssh" {
        return Err(MobileError::invalid(
            "keys can only be exported to SSH hosts",
        ));
    }
    let target = SshTarget {
        host: resolved.host.data.address.clone(),
        port: resolved.port(),
        username: resolved.username().unwrap_or_default(),
    };
    let display = format!("{}@{}:{}", target.username, target.host, target.port);
    let (client, jumps) = connect_resolved(conn, settings, target, Some(&resolved)).await?;
    let out = client
        .exec(
            keychain::EXPORT_COMMAND,
            Some(bytes::Bytes::from(format!("{public}\n"))),
        )
        .await;
    let _ = client.disconnect().await;
    for j in jumps {
        let _ = j.disconnect().await;
    }
    let out = out?;
    let outcome = keychain::export_outcome(out.exit_code, &out.stdout, &out.stderr)?;
    Ok(KeyExportResult {
        added: outcome == keychain::ExportOutcome::Added,
        host_label: resolved.host.data.label.clone(),
        target: display,
    })
}

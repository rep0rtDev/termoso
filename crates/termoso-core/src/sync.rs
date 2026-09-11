//! Sync engine: pushes local changes, pulls remote ones, resolves conflicts,
//! keeps cursors, and listens on the WebSocket for "something changed"
//! nudges.
//!
//! The server only ever sees ciphertext; this module shuffles opaque
//! envelopes between the [`Store`] and the [`ApiClient`]. WebSocket frames
//! carry no payload — they just tell us which REST endpoint to hit next.
//!
//! ```text
//! sync_once
//!   ├─ for every unlocked synced vault
//!   │    ├─ push  dirty rows → PushResult::{Ok, Conflict, Error}
//!   │    └─ pull  cursor → apply_remote (skipping / resolving dirty rows)
//!   ├─ history  push dirty → pull since cursor
//!   └─ logs     upload finished recordings, push deletions → pull metadata
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use termoso_proto::entities::SyncEntity;
use termoso_proto::logs::{CreateLogRequest, UpdateLogRequest};
use termoso_proto::sync::{HistoryPushRequest, MAX_BATCH, PullRequest, PushRequest, PushResult};
use termoso_proto::ws::{ClientMessage, ServerMessage};
use tokio::sync::{Notify, broadcast};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::account;
use crate::api::ApiClient;
use crate::error::{CoreError, Result};
use crate::store::{EntityRow, Store};

const STALE_KEY_VERSION: &str = "stale_key_version";

/// How to resolve an entity that changed both here and on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConflictPolicy {
    /// Keep whichever side has the later `updated_at` (ties → server).
    #[default]
    NewestWins,
    /// Always keep the local payload and overwrite the server.
    LocalWins,
    /// Always take the server copy and drop the local edit.
    ServerWins,
}

/// Engine knobs.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// Conflict resolution.
    pub conflict: ConflictPolicy,
    /// Max entities per push / pull request (server caps at [`MAX_BATCH`]).
    pub batch: usize,
    /// Directory for downloaded session-log bodies. `None` disables body
    /// downloads (metadata still syncs).
    pub log_dir: Option<PathBuf>,
    /// Upload finished recordings from synced vaults.
    pub upload_logs: bool,
    /// Full sync interval while the realtime loop is running.
    pub interval: Duration,
    /// Debounce between a WebSocket nudge and the sync it triggers.
    pub debounce: Duration,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            conflict: ConflictPolicy::NewestWins,
            batch: MAX_BATCH,
            log_dir: None,
            upload_logs: true,
            interval: Duration::from_secs(15 * 60),
            debounce: Duration::from_millis(300),
        }
    }
}

/// What one [`SyncEngine::sync_once`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Entities accepted by the server.
    pub pushed: usize,
    /// Entities applied from the server (including tombstones).
    pub pulled: usize,
    /// Conflicts encountered (however they were resolved).
    pub conflicts: usize,
    /// Per-entity server errors (`id`, `code`). The rows stay dirty.
    pub errors: Vec<(Uuid, String)>,
    /// History entries pushed / pulled.
    pub history: (usize, usize),
    /// Session logs uploaded / metadata rows pulled.
    pub logs: (usize, usize),
}

impl SyncReport {
    /// Anything changed locally that a UI should re-render?
    pub fn changed_locally(&self) -> bool {
        self.pulled > 0 || self.history.1 > 0 || self.logs.1 > 0
    }
}

/// Events for the UI layer.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// A sync pass started.
    Started,
    /// A sync pass finished.
    Finished(SyncReport),
    /// A sync pass failed (transient — will retry).
    Failed(String),
    /// Realtime connection established.
    Connected,
    /// Realtime connection lost; reconnecting.
    Disconnected,
    /// Vault or team membership changed (keys were refreshed).
    VaultsChanged,
    /// Entities in a vault changed on this device (after pull).
    EntitiesChanged {
        /// Vault.
        vault_id: Uuid,
    },
    /// History changed (after pull).
    HistoryChanged,
    /// Session-log list changed (after pull).
    LogsChanged,
    /// Profile / security settings changed on the server.
    AccountChanged,
    /// The server revoked this session. The loop has stopped; the app should
    /// call [`account::sign_out_local`].
    SessionRevoked,
}

/// Sync engine for one signed-in store.
pub struct SyncEngine {
    api: Arc<ApiClient>,
    store: Arc<Store>,
    opts: SyncOptions,
    events: broadcast::Sender<SyncEvent>,
    kick: Notify,
    running: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for SyncEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncEngine")
            .field("api", &self.api)
            .field("opts", &self.opts)
            .finish()
    }
}

impl SyncEngine {
    /// Build an engine. The `ApiClient` must already carry the session token
    /// (see [`account::resume`] / [`account::LoginFlow`]).
    pub fn new(api: Arc<ApiClient>, store: Arc<Store>, opts: SyncOptions) -> Arc<Self> {
        let (events, _) = broadcast::channel(256);
        Arc::new(Self {
            api,
            store,
            opts: SyncOptions {
                batch: opts.batch.clamp(1, MAX_BATCH),
                ..opts
            },
            events,
            kick: Notify::new(),
            running: tokio::sync::Mutex::new(()),
        })
    }

    /// Store this engine syncs.
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// API client this engine uses.
    pub fn api(&self) -> &Arc<ApiClient> {
        &self.api
    }

    /// Subscribe to [`SyncEvent`]s.
    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.events.subscribe()
    }

    /// Ask the realtime loop to sync soon (after a local edit).
    pub fn request_sync(&self) {
        self.kick.notify_one();
    }

    fn emit(&self, ev: SyncEvent) {
        let _ = self.events.send(ev);
    }

    /// One full pass: every vault, history and logs. Serialised — concurrent
    /// callers wait for the running pass.
    pub async fn sync_once(&self) -> Result<SyncReport> {
        let _guard = self.running.lock().await;
        self.emit(SyncEvent::Started);
        match self.sync_all().await {
            Ok(r) => {
                self.emit(SyncEvent::Finished(r.clone()));
                Ok(r)
            }
            Err(e) => {
                self.emit(SyncEvent::Failed(e.to_string()));
                Err(e)
            }
        }
    }

    async fn sync_all(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        let vaults = self.synced_vault_ids()?;
        for vault_id in &vaults {
            self.sync_vault_into(*vault_id, &mut report).await?;
        }
        self.pull_entities_into(&vaults, &mut report).await?;
        self.sync_history_into(&mut report).await?;
        self.sync_logs_into(&mut report).await?;
        Ok(report)
    }

    fn synced_vault_ids(&self) -> Result<Vec<Uuid>> {
        Ok(self
            .store
            .vaults()?
            .into_iter()
            .filter(|v| v.kind.is_synced() && v.unlocked)
            .map(|v| v.id)
            .collect())
    }

    /// Push then pull a single vault.
    pub async fn sync_vault(&self, vault_id: Uuid) -> Result<SyncReport> {
        let _guard = self.running.lock().await;
        let mut report = SyncReport::default();
        self.sync_vault_into(vault_id, &mut report).await?;
        self.pull_entities_into(&[vault_id], &mut report).await?;
        Ok(report)
    }

    async fn sync_vault_into(&self, vault_id: Uuid, report: &mut SyncReport) -> Result<()> {
        // Conflicts resolved in favour of the local copy are re-pushed at
        // once; bounded so two devices fighting cannot spin forever.
        for _ in 0..3 {
            let again = self.push_entities_into(vault_id, report).await?;
            if !again {
                break;
            }
        }
        Ok(())
    }

    /// Push dirty rows of one vault. Returns `true` when local-wins conflicts
    /// left rows that should be pushed again.
    async fn push_entities_into(&self, vault_id: Uuid, report: &mut SyncReport) -> Result<bool> {
        let mut retry = false;
        let mut stale_key = false;
        let rows = self.store.dirty_rows(vault_id)?;
        for chunk in rows.chunks(self.opts.batch) {
            let by_id: HashMap<Uuid, &EntityRow> = chunk.iter().map(|r| (r.id, r)).collect();
            let mut req = PushRequest {
                changes: Vec::new(),
                deletes: Vec::new(),
            };
            for r in chunk {
                if r.deleted {
                    if r.version > 0 {
                        req.deletes.push(r.to_delete());
                    } else {
                        // Created and deleted before the server saw it.
                        self.store.mark_pushed(r.id, 0, 0)?;
                    }
                } else {
                    req.changes.push(r.to_change());
                }
            }
            if req.changes.is_empty() && req.deletes.is_empty() {
                continue;
            }
            let resp = self.api.sync_push(&req).await?;
            for res in resp.results {
                match res {
                    PushResult::Ok { id, version, seq } => {
                        self.store.mark_pushed(id, version, seq)?;
                        report.pushed += 1;
                    }
                    PushResult::Conflict { id, server } => {
                        report.conflicts += 1;
                        let Some(local) = by_id.get(&id) else {
                            continue;
                        };
                        if self.resolve_conflict(local, &server)? {
                            retry = true;
                        }
                    }
                    PushResult::Error { id, code } => {
                        if code == termoso_proto::error::codes::NOT_FOUND {
                            // The server never had it (or purged the tombstone).
                            match by_id.get(&id) {
                                Some(r) if r.deleted => self.store.mark_pushed(id, 0, 0)?,
                                Some(_) => {
                                    self.store.rebase_local(id, 0, 0)?;
                                    retry = true;
                                }
                                None => {}
                            }
                            continue;
                        }
                        if code == STALE_KEY_VERSION {
                            stale_key = true;
                        }
                        report.errors.push((id, code));
                    }
                }
            }
        }
        if stale_key {
            // Someone rotated the vault key; pick up the new one and let the
            // next pass re-encrypt / re-pull.
            account::refresh_vaults(&self.api, &self.store).await?;
            self.emit(SyncEvent::VaultsChanged);
        }
        Ok(retry)
    }

    /// Apply the configured policy. Returns `true` if the local copy should be
    /// pushed again.
    fn resolve_conflict(&self, local: &EntityRow, server: &SyncEntity) -> Result<bool> {
        let keep_local = match self.opts.conflict {
            ConflictPolicy::LocalWins => true,
            ConflictPolicy::ServerWins => false,
            ConflictPolicy::NewestWins => local.updated_at > server.updated_at,
        };
        if keep_local {
            self.store
                .rebase_local(local.id, server.version, server.seq)?;
            Ok(true)
        } else {
            self.store.apply_remote(server)?;
            Ok(false)
        }
    }

    /// Pull the given vaults from their cursors until the server has nothing
    /// more.
    pub async fn pull_entities(&self, vault_ids: &[Uuid]) -> Result<SyncReport> {
        let _guard = self.running.lock().await;
        let mut report = SyncReport::default();
        self.pull_entities_into(vault_ids, &mut report).await?;
        Ok(report)
    }

    async fn pull_entities_into(&self, vault_ids: &[Uuid], report: &mut SyncReport) -> Result<()> {
        if vault_ids.is_empty() {
            return Ok(());
        }
        let mut cursors: HashMap<Uuid, i64> = HashMap::new();
        for id in vault_ids {
            cursors.insert(*id, self.store.vault(*id)?.cursor);
        }
        let mut touched: Vec<Uuid> = Vec::new();
        loop {
            let resp = self
                .api
                .sync_pull(&PullRequest {
                    cursors: cursors.clone(),
                    limit: Some(self.opts.batch as u32),
                })
                .await?;
            for e in &resp.entities {
                if self.apply_pulled(e)? {
                    report.pulled += 1;
                    if !touched.contains(&e.vault_id) {
                        touched.push(e.vault_id);
                    }
                }
            }
            for (vault_id, cursor) in &resp.cursors {
                if let Some(local) = cursors.get_mut(vault_id)
                    && *cursor > *local
                {
                    *local = *cursor;
                    self.store.set_vault_cursor(*vault_id, *cursor)?;
                }
            }
            if !resp.has_more {
                break;
            }
        }
        for vault_id in touched {
            self.emit(SyncEvent::EntitiesChanged { vault_id });
        }
        Ok(())
    }

    /// Apply one pulled entity, protecting unpushed local edits. Returns
    /// whether the local database changed.
    fn apply_pulled(&self, e: &SyncEntity) -> Result<bool> {
        let Some(local) = self.store.row(e.id)? else {
            if e.deleted {
                return Ok(false);
            }
            self.store.apply_remote(e)?;
            return Ok(true);
        };
        if !local.dirty {
            if local.version >= e.version && !e.deleted {
                return Ok(false);
            }
            self.store.apply_remote(e)?;
            return Ok(true);
        }
        if local.version >= e.version {
            // Echo of our own push (or older) — the dirty row is newer.
            return Ok(false);
        }
        let resolved_remote = !self.resolve_conflict(&local, e)?;
        Ok(resolved_remote)
    }

    // ───────────────────────────── history ─────────────────────────────

    /// Push and pull command / connection history.
    pub async fn sync_history(&self) -> Result<SyncReport> {
        let _guard = self.running.lock().await;
        let mut report = SyncReport::default();
        self.sync_history_into(&mut report).await?;
        Ok(report)
    }

    async fn sync_history_into(&self, report: &mut SyncReport) -> Result<()> {
        let account = self.store.account()?.ok_or(CoreError::NotSignedIn)?;
        let dirty = self.store.dirty_history()?;
        for chunk in dirty.chunks(self.opts.batch) {
            let resp = self
                .api
                .history_push(&HistoryPushRequest {
                    entries: chunk.to_vec(),
                })
                .await?;
            for e in &resp.entries {
                self.store.mark_history_pushed(&[e.id], e.seq)?;
            }
            report.history.0 += resp.entries.len();
        }
        let mut since = account.history_cursor;
        let mut changed = false;
        loop {
            let resp = self.api.history_pull(since, self.opts.batch as u32).await?;
            let applied = self.store.apply_remote_history(&resp.entries)?;
            if applied > 0 {
                report.history.1 += applied;
                changed = true;
            }
            let next = resp
                .entries
                .iter()
                .map(|e| e.seq)
                .max()
                .unwrap_or(since)
                .max(resp.since);
            if next > since {
                since = next;
                self.store.set_account_cursors(Some(since), None)?;
            }
            if !resp.has_more || resp.entries.is_empty() {
                break;
            }
        }
        if changed {
            self.emit(SyncEvent::HistoryChanged);
        }
        Ok(())
    }

    // ───────────────────────────── session logs ─────────────────────────────

    /// Upload finished recordings, push deletions and pull metadata.
    pub async fn sync_logs(&self) -> Result<SyncReport> {
        let _guard = self.running.lock().await;
        let mut report = SyncReport::default();
        self.sync_logs_into(&mut report).await?;
        Ok(report)
    }

    async fn sync_logs_into(&self, report: &mut SyncReport) -> Result<()> {
        let account = self.store.account()?.ok_or(CoreError::NotSignedIn)?;
        if self.opts.upload_logs {
            for row in self.store.logs_to_upload()? {
                match self.upload_log(&row).await {
                    Ok(seq) => {
                        self.store.mark_log_uploaded(row.id, seq)?;
                        report.logs.0 += 1;
                    }
                    // Storage-side refusals are per-log and must not stall
                    // the rest of the pass.
                    Err(e) if matches!(&e, CoreError::Api { status, .. } if (400..500).contains(status) || *status == 507) =>
                    {
                        report.errors.push((row.id, api_code(&e)));
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        for row in self.store.logs_to_delete_remote()? {
            match self.api.delete_log(row.id).await {
                Ok(()) => self.store.forget_log(row.id)?,
                Err(e) if e.is_api_code(termoso_proto::error::codes::NOT_FOUND) => {
                    self.store.forget_log(row.id)?
                }
                Err(e) => return Err(e),
            }
        }
        let mut since = account.logs_cursor;
        let mut changed = false;
        loop {
            let resp = self.api.logs(since, self.opts.batch as u32).await?;
            if !resp.logs.is_empty() {
                self.store.apply_remote_logs(&resp.logs)?;
                report.logs.1 += resp.logs.len();
                changed = true;
            }
            let next = resp
                .logs
                .iter()
                .map(|l| l.seq)
                .max()
                .unwrap_or(since)
                .max(resp.since);
            if next > since {
                since = next;
                self.store.set_account_cursors(None, Some(since))?;
            }
            if !resp.has_more || resp.logs.is_empty() {
                break;
            }
        }
        if changed {
            self.emit(SyncEvent::LogsChanged);
        }
        Ok(())
    }

    async fn upload_log(&self, row: &crate::store::LogRow) -> Result<i64> {
        let body = self.store.log_body_ciphertext(row.id)?;
        let size = body.len() as i64;
        let created = self
            .api
            .create_log(&CreateLogRequest {
                id: row.id,
                vault_id: row.vault_id,
                meta: row.meta.clone(),
                key_version: row.key_version,
                size_bytes: size,
            })
            .await;
        match created {
            Ok(target) => self.api.upload_log_object(&target, body).await?,
            // Registered on an earlier pass that died before completion; the
            // object may already be there, so just try to complete it.
            Err(e) if e.is_api_code(termoso_proto::error::codes::CONFLICT) => {}
            Err(e) => return Err(e),
        }
        let log = self
            .api
            .update_log(
                row.id,
                &UpdateLogRequest {
                    meta: None,
                    size_bytes: Some(size),
                },
            )
            .await?;
        Ok(log.seq)
    }

    /// Download a recording body that exists on the server but not here.
    /// Requires [`SyncOptions::log_dir`].
    pub async fn download_log(&self, id: Uuid) -> Result<PathBuf> {
        let dir = self
            .opts
            .log_dir
            .as_ref()
            .ok_or_else(|| CoreError::Invalid("log_dir not configured".into()))?;
        let target = self.api.log_download_url(id).await?;
        let body = self.api.download_log_object(&target).await?;
        self.store.cache_log_body(id, &body, dir)
    }

    // ───────────────────────────── realtime ─────────────────────────────

    /// Run until cancelled: initial sync, WebSocket nudges → targeted syncs,
    /// periodic full syncs, reconnect with backoff. Stops on its own when the
    /// server revokes the session (after emitting
    /// [`SyncEvent::SessionRevoked`]).
    pub async fn run(self: Arc<Self>, cancel: CancellationToken) {
        let mut backoff = Duration::from_secs(1);
        loop {
            if cancel.is_cancelled() {
                return;
            }
            match self.session(&cancel).await {
                Ok(Stop::Cancelled) => return,
                Ok(Stop::Revoked) => {
                    self.emit(SyncEvent::SessionRevoked);
                    return;
                }
                Ok(Stop::Disconnected) => {
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    if e.is_unauthorized() {
                        self.emit(SyncEvent::SessionRevoked);
                        return;
                    }
                    tracing::debug!("realtime session ended: {e}");
                }
            }
            self.emit(SyncEvent::Disconnected);
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(backoff) => {}
            }
            backoff = (backoff * 2).min(Duration::from_secs(60));
        }
    }

    async fn session(&self, cancel: &CancellationToken) -> Result<Stop> {
        let token = self.api.token().ok_or(CoreError::NotSignedIn)?;
        let url = self.api.ws_url()?;
        let (mut ws, _) = tokio::select! {
            _ = cancel.cancelled() => return Ok(Stop::Cancelled),
            r = tokio_tungstenite::connect_async(url.as_str()) => r.map_err(|e| CoreError::Ws(e.to_string()))?,
        };
        let auth = serde_json::to_string(&ClientMessage::Auth { token })?;
        ws.send(Message::Text(auth.into()))
            .await
            .map_err(|e| CoreError::Ws(e.to_string()))?;

        // First frame is Hello (or an error) — treat anything else as a
        // protocol violation.
        let hello = tokio::select! {
            _ = cancel.cancelled() => return Ok(Stop::Cancelled),
            m = ws.next() => m,
        };
        match decode(hello)? {
            Some(ServerMessage::Hello { vault_ids, .. }) => {
                self.emit(SyncEvent::Connected);
                if self.vault_set_differs(&vault_ids)? {
                    account::refresh_vaults(&self.api, &self.store).await?;
                    self.emit(SyncEvent::VaultsChanged);
                }
            }
            Some(ServerMessage::Error { code, message }) => {
                if code == "unauthorized" || code == "auth_required" {
                    return Ok(Stop::Revoked);
                }
                return Err(CoreError::Ws(format!("{code}: {message}")));
            }
            Some(ServerMessage::SessionRevoked) => return Ok(Stop::Revoked),
            Some(other) => return Err(CoreError::Ws(format!("unexpected first frame {other:?}"))),
            None => return Ok(Stop::Disconnected),
        }

        // Catch up on anything missed while offline.
        if let Err(e) = self.sync_once().await {
            if e.is_unauthorized() {
                return Ok(Stop::Revoked);
            }
            tracing::warn!("initial sync failed: {e}");
        }

        let my_device = self.store.account()?.map(|a| a.device_id);
        let mut pending = Pending::default();
        let mut ping = tokio::time::interval(Duration::from_secs(30));
        ping.tick().await;
        let mut full = tokio::time::interval(self.opts.interval);
        full.tick().await;
        let debounce = tokio::time::sleep(Duration::MAX);
        tokio::pin!(debounce);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = ws.close(None).await;
                    return Ok(Stop::Cancelled);
                }
                _ = ping.tick() => {
                    let msg = serde_json::to_string(&ClientMessage::Ping)?;
                    if ws.send(Message::Text(msg.into())).await.is_err() {
                        return Ok(Stop::Disconnected);
                    }
                }
                _ = full.tick() => {
                    pending.full = true;
                    debounce.as_mut().reset(tokio::time::Instant::now());
                }
                _ = self.kick.notified() => {
                    pending.full = true;
                    debounce.as_mut().reset(tokio::time::Instant::now() + self.opts.debounce);
                }
                _ = &mut debounce => {
                    let work = std::mem::take(&mut pending);
                    debounce.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(86_400 * 365));
                    if let Err(e) = self.flush(work).await {
                        if e.is_unauthorized() {
                            return Ok(Stop::Revoked);
                        }
                        tracing::warn!("sync after notification failed: {e}");
                    }
                }
                frame = ws.next() => {
                    let Some(msg) = decode(frame)? else {
                        return Ok(Stop::Disconnected);
                    };
                    match msg {
                        ServerMessage::Hello { .. } | ServerMessage::Pong => {}
                        ServerMessage::VaultChanged { vault_id, seq, device_id } => {
                            if device_id.is_some() && device_id == my_device {
                                continue;
                            }
                            let known = self.store.vault(vault_id).ok();
                            match known {
                                Some(v) if v.cursor >= seq => continue,
                                Some(_) => pending.vaults.push(vault_id),
                                None => pending.vault_list = true,
                            }
                        }
                        ServerMessage::HistoryChanged { seq } => {
                            let cur = self.store.account()?.map_or(0, |a| a.history_cursor);
                            if seq > cur {
                                pending.history = true;
                            }
                        }
                        ServerMessage::LogsChanged { seq } => {
                            let cur = self.store.account()?.map_or(0, |a| a.logs_cursor);
                            if seq > cur {
                                pending.logs = true;
                            }
                        }
                        ServerMessage::VaultsUpdated | ServerMessage::TeamsUpdated => {
                            pending.vault_list = true;
                        }
                        ServerMessage::AccountUpdated => {
                            pending.account = true;
                        }
                        ServerMessage::SessionRevoked => return Ok(Stop::Revoked),
                        ServerMessage::Error { code, message } => {
                            tracing::warn!("server ws error {code}: {message}");
                        }
                    }
                    if pending.any() {
                        debounce.as_mut().reset(tokio::time::Instant::now() + self.opts.debounce);
                    }
                }
            }
        }
    }

    fn vault_set_differs(&self, server: &[Uuid]) -> Result<bool> {
        let local: Vec<Uuid> = self
            .store
            .vaults()?
            .into_iter()
            .filter(|v| v.kind.is_synced())
            .map(|v| v.id)
            .collect();
        Ok(local.len() != server.len() || server.iter().any(|id| !local.contains(id)))
    }

    async fn flush(&self, work: Pending) -> Result<()> {
        if work.account {
            let me = self.api.account().await?;
            self.store.update_account_profile(
                &me.user.email,
                me.user.display_name.as_deref(),
                me.user.is_admin,
            )?;
            self.emit(SyncEvent::AccountChanged);
        }
        if work.vault_list {
            account::refresh_vaults(&self.api, &self.store).await?;
            self.emit(SyncEvent::VaultsChanged);
        }
        if work.full || work.vault_list {
            self.sync_once().await?;
            return Ok(());
        }
        let _guard = self.running.lock().await;
        let mut report = SyncReport::default();
        if !work.vaults.is_empty() {
            let mut ids = work.vaults;
            ids.sort();
            ids.dedup();
            for id in &ids {
                self.sync_vault_into(*id, &mut report).await?;
            }
            self.pull_entities_into(&ids, &mut report).await?;
        }
        if work.history {
            self.sync_history_into(&mut report).await?;
        }
        if work.logs {
            self.sync_logs_into(&mut report).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Pending {
    full: bool,
    vault_list: bool,
    account: bool,
    history: bool,
    logs: bool,
    vaults: Vec<Uuid>,
}

impl Pending {
    fn any(&self) -> bool {
        self.full
            || self.vault_list
            || self.account
            || self.history
            || self.logs
            || !self.vaults.is_empty()
    }
}

enum Stop {
    Cancelled,
    Revoked,
    Disconnected,
}

type WsFrame = Option<std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>;

/// `Ok(None)` = connection closed.
fn decode(frame: WsFrame) -> Result<Option<ServerMessage>> {
    match frame {
        None | Some(Ok(Message::Close(_))) => Ok(None),
        Some(Err(e)) => Err(CoreError::Ws(e.to_string())),
        Some(Ok(Message::Text(t))) => Ok(Some(serde_json::from_str(&t)?)),
        Some(Ok(Message::Binary(b))) => Ok(Some(serde_json::from_slice(&b)?)),
        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {
            Ok(Some(ServerMessage::Pong))
        }
    }
}

fn api_code(e: &CoreError) -> String {
    match e {
        CoreError::Api { code, .. } => code.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_within_server_limits() {
        let o = SyncOptions::default();
        assert!(o.batch <= MAX_BATCH);
        assert_eq!(o.conflict, ConflictPolicy::NewestWins);
    }

    #[test]
    fn decode_handles_control_frames() {
        assert!(decode(None).unwrap().is_none());
        assert!(matches!(
            decode(Some(Ok(Message::Ping(vec![].into())))).unwrap(),
            Some(ServerMessage::Pong)
        ));
        let m = decode(Some(Ok(Message::Text(
            r#"{"type":"history_changed","seq":5}"#.into(),
        ))))
        .unwrap();
        assert!(matches!(m, Some(ServerMessage::HistoryChanged { seq: 5 })));
    }
}

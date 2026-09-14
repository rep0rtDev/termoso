//! The one object Kotlin holds: an open encrypted profile. Everything is
//! synchronous and cheap except key generation and connecting, which the
//! caller runs off the main thread (generation) or which run on the Rust
//! runtime and report back through the listener (sessions).

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use termoso_client::hosts::{self, CopyCredentials};
use termoso_client::keychain;
use termoso_core::hostkey::KnownHosts;
use termoso_core::model::ResolvedHost;
use termoso_core::ssh::SshTarget;
use termoso_core::store::Store;
use termoso_crypto::keys::SymmetricKey;
use zeroize::Zeroizing;

use crate::account::{
    AccountCard, AccountRuntime, AccountStatus, DeviceCard, LoginForm, LoginOutcome, MfaMethod,
    RegisterForm, Registered, ServerCard, SyncListener, SyncStatus,
};
use crate::dto::*;
use crate::error::{MobileError, Result};
use crate::forward::{self, PfRuleDraft, PfRuleItem, PfTunnel, TunnelLaunch, TunnelListener};
use crate::session::{Launch, SessionListener, SshSession, TerminalOptions};
use crate::settings::MobileSettings;
use crate::sftp::{SftpLaunch, SftpListener, SftpSession};

const DB_FILE: &str = "vault.db";

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("termoso-rt")
        .enable_all()
        .build()
        .expect("tokio runtime")
});

/// Route Rust `tracing` output to logcat (tag `termoso`) on Android, stderr
/// elsewhere. Idempotent. Never logs secrets: the core masks them.
#[uniffi::export]
pub fn init_logging(verbose: bool) {
    use tracing_subscriber::prelude::*;
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let filter = tracing_subscriber::filter::LevelFilter::from_level(level);
    #[cfg(target_os = "android")]
    let layer = tracing_android::layer("termoso").ok();
    #[cfg(not(target_os = "android"))]
    let layer = Some(tracing_subscriber::fmt::layer().with_target(false));
    if let Some(layer) = layer {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(layer)
            .try_init();
    }
}

/// Crate version (`CARGO_PKG_VERSION`).
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// A fresh 32-byte master key for a new profile. The caller wraps it with
/// the Android Keystore and hands it back to [`TermosoApp::open`] on every
/// launch; Rust never persists it.
#[uniffi::export]
pub fn generate_master_key() -> Vec<u8> {
    SymmetricKey::generate().as_bytes().to_vec()
}

/// Whether `profile_dir` already contains a vault.
#[uniffi::export]
pub fn profile_exists(profile_dir: String) -> bool {
    Path::new(&profile_dir).join(DB_FILE).is_file()
}

/// Parse `user@host:port`, `ssh://user@host:port` or plain `host` into a
/// target (defaults: `root`, 22). Errors on an empty host.
#[uniffi::export]
pub fn parse_target(input: String) -> Result<QuickTarget> {
    let s = input.trim().trim_start_matches("ssh://");
    let s = s.trim_end_matches('/');
    let (user, rest) = match s.rsplit_once('@') {
        Some((u, r)) if !u.is_empty() => (Some(u.to_string()), r),
        _ => (None, s),
    };
    let (host, port) = if let Some(stripped) = rest.strip_prefix('[') {
        // [v6]:port
        match stripped.split_once(']') {
            Some((h, p)) => (h.to_string(), p.trim_start_matches(':').parse::<u16>().ok()),
            None => (stripped.to_string(), None),
        }
    } else if rest.matches(':').count() == 1 {
        let (h, p) = rest.split_once(':').unwrap_or((rest, ""));
        (h.to_string(), p.parse::<u16>().ok())
    } else {
        (rest.to_string(), None)
    };
    if host.is_empty() {
        return Err(MobileError::invalid("host is empty"));
    }
    Ok(QuickTarget {
        host,
        port: port.unwrap_or(22),
        username: user.unwrap_or_else(|| "root".into()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct QuickTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
}

/// Probe a server before sign-in. Nothing is contacted until the user
/// picked it.
#[uniffi::export]
pub fn server_info(url: String) -> Result<ServerCard> {
    RUNTIME.block_on(AccountRuntime::server_info(&url))
}

/// An open profile.
#[derive(uniffi::Object)]
pub struct TermosoApp {
    store: Arc<Store>,
    profile_dir: PathBuf,
    account: Arc<AccountRuntime>,
}

impl Drop for TermosoApp {
    fn drop(&mut self) {
        self.account.shutdown(RUNTIME.handle());
    }
}

#[uniffi::export]
impl TermosoApp {
    /// Open (or create) the encrypted vault in `profile_dir` with the
    /// 32-byte master key from the Keystore.
    #[uniffi::constructor]
    pub fn open(profile_dir: String, master_key: Vec<u8>) -> Result<Arc<Self>> {
        let key: [u8; 32] = master_key
            .as_slice()
            .try_into()
            .map_err(|_| MobileError::invalid("master key must be 32 bytes"))?;
        let master = SymmetricKey::from_bytes(key);
        let dir = PathBuf::from(profile_dir);
        std::fs::create_dir_all(&dir)?;
        let store = Arc::new(Store::open(&dir.join(DB_FILE), master)?);
        Ok(Arc::new(Self {
            account: AccountRuntime::new(store.clone()),
            store,
            profile_dir: dir,
        }))
    }

    pub fn profile_dir(&self) -> String {
        self.profile_dir.to_string_lossy().into_owned()
    }

    /// Stable per-install id (not sent anywhere in phase 1).
    pub fn device_id(&self) -> Result<String> {
        Ok(self.store.device_id()?.to_string())
    }

    // ---- vaults -------------------------------------------------------

    pub fn vaults(&self) -> Result<Vec<VaultInfo>> {
        Ok(self
            .store
            .vaults()?
            .into_iter()
            .map(VaultInfo::from)
            .collect())
    }

    pub fn local_vault(&self) -> Result<VaultInfo> {
        Ok(self.store.local_vault()?.into())
    }

    // ---- hosts --------------------------------------------------------

    /// Hosts of one vault, or of all vaults when `vault_id` is `None`.
    pub fn hosts(&self, vault_id: Option<String>) -> Result<Vec<HostItem>> {
        let vault = parse_opt_id(&vault_id)?;
        Ok(hosts::cards(&self.store, vault)?
            .into_iter()
            .map(HostItem::from)
            .collect())
    }

    pub fn host(&self, id: String) -> Result<HostItem> {
        let id = parse_id(&id)?;
        hosts::cards(&self.store, None)?
            .into_iter()
            .find(|c| c.id == id)
            .map(HostItem::from)
            .ok_or_else(|| MobileError::not_found("host"))
    }

    pub fn groups(&self, vault_id: Option<String>) -> Result<Vec<GroupItem>> {
        let vault = parse_opt_id(&vault_id)?;
        Ok(hosts::groups(&self.store, vault)?
            .into_iter()
            .map(GroupItem::from)
            .collect())
    }

    pub fn tags(&self, vault_id: Option<String>) -> Result<Vec<TagItem>> {
        let vault = parse_opt_id(&vault_id)?;
        Ok(hosts::tags(&self.store, vault)?
            .into_iter()
            .map(TagItem::from)
            .collect())
    }

    /// Editor model for an existing host (no password: `has_password` says
    /// whether one is stored).
    pub fn host_draft(&self, id: String) -> Result<HostDraft> {
        Ok(hosts::form(&self.store, parse_id(&id)?)?.into())
    }

    /// Empty editor model for a new host.
    pub fn new_host_draft(&self, vault_id: String, group_id: Option<String>) -> Result<HostDraft> {
        Ok(HostDraft::blank(
            parse_id(&vault_id)?,
            parse_opt_id(&group_id)?,
        ))
    }

    /// What a host in `group_id` inherits (placeholders for the editor).
    pub fn inherited(&self, group_id: Option<String>) -> Result<InheritedInfo> {
        Ok(hosts::inherited(&self.store, parse_opt_id(&group_id)?)?.into())
    }

    /// Create or update a host. Sections the mobile editor does not show
    /// are preserved on update.
    pub fn save_host(&self, draft: HostDraft) -> Result<HostItem> {
        if draft.label.trim().is_empty() && draft.address.trim().is_empty() {
            return Err(MobileError::invalid("address is required"));
        }
        let base = match parse_opt_id(&draft.id)? {
            Some(id) => hosts::form(&self.store, id)?,
            None => blank_form(parse_id(&draft.vault_id)?),
        };
        let form = draft.apply(base)?;
        Ok(hosts::save(&self.store, &form)?.into())
    }

    pub fn delete_host(&self, id: String) -> Result<()> {
        Ok(hosts::delete(&self.store, parse_id(&id)?)?)
    }

    pub fn delete_hosts(&self, ids: Vec<String>) -> Result<()> {
        for id in parse_ids(&ids)? {
            hosts::delete(&self.store, id)?;
        }
        Ok(())
    }

    pub fn duplicate_host(&self, id: String) -> Result<HostItem> {
        Ok(hosts::duplicate(&self.store, parse_id(&id)?)?.into())
    }

    pub fn move_hosts(&self, ids: Vec<String>, group_id: Option<String>) -> Result<()> {
        Ok(hosts::move_hosts(
            &self.store,
            &parse_ids(&ids)?,
            parse_opt_id(&group_id)?,
        )?)
    }

    /// Copy hosts into another vault. `with_credentials` copies keys /
    /// passwords along (team vault sharing); otherwise only the address book.
    pub fn copy_hosts_to_vault(
        &self,
        ids: Vec<String>,
        vault_id: String,
        with_credentials: bool,
    ) -> Result<Vec<String>> {
        let creds = if with_credentials {
            CopyCredentials::Shared
        } else {
            CopyCredentials::Personal
        };
        Ok(
            hosts::copy_to_vault(&self.store, &parse_ids(&ids)?, parse_id(&vault_id)?, creds)?
                .into_iter()
                .map(|u| u.to_string())
                .collect(),
        )
    }

    pub fn save_group(
        &self,
        vault_id: String,
        id: Option<String>,
        label: String,
        parent_id: Option<String>,
    ) -> Result<GroupItem> {
        Ok(hosts::save_group(
            &self.store,
            parse_id(&vault_id)?,
            parse_opt_id(&id)?,
            &label,
            parse_opt_id(&parent_id)?,
        )?
        .into())
    }

    /// Delete a group; hosts and sub-groups move up one level.
    pub fn delete_group(&self, id: String) -> Result<()> {
        Ok(hosts::delete_group(&self.store, parse_id(&id)?)?)
    }

    /// Find-or-create a tag by label in `vault_id`.
    pub fn create_tag(&self, vault_id: String, label: String) -> Result<TagItem> {
        let vault = parse_id(&vault_id)?;
        let label = label.trim();
        if label.is_empty() {
            return Err(MobileError::invalid("tag label is empty"));
        }
        if let Some(existing) = hosts::tags(&self.store, Some(vault))?
            .into_iter()
            .find(|t| t.label.eq_ignore_ascii_case(label))
        {
            return Ok(existing.into());
        }
        let id = self.store.insert(
            vault,
            &termoso_core::model::Tag {
                label: label.to_string(),
                color: None,
            },
        )?;
        hosts::tags(&self.store, Some(vault))?
            .into_iter()
            .find(|t| t.id == id)
            .map(TagItem::from)
            .ok_or_else(|| MobileError::not_found("tag"))
    }

    pub fn save_tag(&self, id: String, label: String) -> Result<TagItem> {
        Ok(hosts::tag_update(&self.store, parse_id(&id)?, label, None)?.into())
    }

    pub fn delete_tag(&self, id: String) -> Result<()> {
        Ok(hosts::tag_delete(&self.store, parse_id(&id)?)?)
    }

    // ---- keychain -----------------------------------------------------

    pub fn keys(&self, vault_id: Option<String>) -> Result<Vec<KeyItem>> {
        let vault = parse_opt_id(&vault_id)?;
        Ok(keychain::keys_list(&self.store, vault)?
            .into_iter()
            .map(KeyItem::from)
            .collect())
    }

    /// Generate a key pair into the vault. RSA 4096 takes a few seconds on
    /// a phone: call off the main thread.
    pub fn generate_key(&self, draft: KeyGenerateDraft) -> Result<KeyItem> {
        let form = keychain::GenerateForm {
            vault_id: parse_id(&draft.vault_id)?,
            label: draft.label,
            algorithm: draft.algorithm.into(),
            comment: draft.comment,
            passphrase: draft.passphrase,
            remember_passphrase: draft.remember_passphrase,
        };
        Ok(keychain::generate(&self.store, &form)?.into())
    }

    /// Inspect pasted private-key text before importing.
    pub fn inspect_private_key(&self, text: String) -> Result<KeyPreview> {
        Ok(keychain::inspect_private(&text)?.into())
    }

    pub fn import_key(&self, draft: KeyImportDraft) -> Result<KeyItem> {
        let form = keychain::ImportForm {
            vault_id: parse_id(&draft.vault_id)?,
            label: draft.label,
            private_key: draft.private_key,
            passphrase: draft.passphrase,
            remember_passphrase: draft.remember_passphrase,
            certificate: draft.certificate,
        };
        Ok(keychain::import(&self.store, &form)?.into())
    }

    pub fn rename_key(&self, id: String, label: String) -> Result<KeyItem> {
        Ok(keychain::rename(&self.store, parse_id(&id)?, &label)?.into())
    }

    /// `authorized_keys` line for the key.
    pub fn public_key(&self, id: String) -> Result<String> {
        Ok(keychain::public_key(&self.store, parse_id(&id)?)?)
    }

    /// Export the private key as OpenSSH text — the only call that returns
    /// private material; the UI asks for confirmation first. `passphrase`
    /// unlocks the stored key when it is not remembered; `export_passphrase`
    /// re-encrypts the export (`None` = plain).
    pub fn export_private_key(
        &self,
        id: String,
        passphrase: Option<String>,
        export_passphrase: Option<String>,
    ) -> Result<String> {
        let text: Zeroizing<String> =
            keychain::export(&self.store, parse_id(&id)?, passphrase, export_passphrase)?;
        Ok(text.to_string())
    }

    pub fn change_key_passphrase(
        &self,
        id: String,
        current: Option<String>,
        next: Option<String>,
        remember: bool,
    ) -> Result<KeyItem> {
        Ok(
            keychain::change_passphrase(&self.store, parse_id(&id)?, current, next, remember)?
                .into(),
        )
    }

    pub fn set_key_certificate(&self, id: String, certificate: Option<String>) -> Result<KeyItem> {
        Ok(keychain::set_certificate(&self.store, parse_id(&id)?, certificate)?.into())
    }

    pub fn delete_key(&self, id: String) -> Result<()> {
        Ok(keychain::delete(&self.store, parse_id(&id)?)?)
    }

    pub fn identities(&self, vault_id: Option<String>) -> Result<Vec<IdentityItem>> {
        let vault = parse_opt_id(&vault_id)?;
        Ok(keychain::identities(&self.store, vault)?
            .into_iter()
            .map(IdentityItem::from)
            .collect())
    }

    pub fn save_identity(&self, draft: IdentityDraft) -> Result<IdentityItem> {
        let mut form = draft.into_form()?;
        if let Some(id) = form.id {
            // Keep the certificate the mobile editor does not show.
            if let Some(existing) = keychain::identities(&self.store, None)?
                .into_iter()
                .find(|i| i.id == id)
            {
                form.ssh_certificate_id = existing.ssh_certificate_id;
                form.ssh_id = existing.ssh_id;
                form.ssh_id_key_type = existing.ssh_id_key_type;
            }
        }
        Ok(keychain::save_identity(&self.store, &form)?.into())
    }

    pub fn delete_identity(&self, id: String) -> Result<()> {
        Ok(keychain::delete_identity(&self.store, parse_id(&id)?)?)
    }

    // ---- known hosts --------------------------------------------------

    pub fn known_hosts(&self) -> Result<Vec<KnownHostItem>> {
        let kh = KnownHosts::new(self.store.clone(), self.store.local_vault()?.id);
        Ok(kh
            .all()?
            .into_iter()
            .map(|e| KnownHostItem {
                id: e.id.to_string(),
                hostname: e.data.hostname,
                key_type: e.data.key_type,
                fingerprint: e.data.fingerprint,
                updated_at: millis(e.updated_at),
            })
            .collect())
    }

    pub fn forget_known_host(&self, id: String) -> Result<()> {
        Ok(self.store.delete(parse_id(&id)?)?)
    }

    // ---- history ------------------------------------------------------

    pub fn history(&self, limit: u32) -> Result<Vec<HistoryItem>> {
        Ok(self
            .store
            .connections(limit.clamp(1, 1000) as usize)?
            .into_iter()
            .map(|h| HistoryItem {
                id: h.id.to_string(),
                host_id: h.data.host_id.map(|u| u.to_string()),
                label: h.data.label,
                target: h.data.target,
                protocol: h.data.protocol,
                started_at: millis(h.created_at),
                duration_secs: h.data.duration_secs,
                error: h.data.error,
            })
            .collect())
    }

    pub fn clear_history(&self) -> Result<()> {
        Ok(self
            .store
            .clear_history(Some(termoso_proto::sync::HistoryKind::Connection))?)
    }

    // ---- settings -----------------------------------------------------

    pub fn settings(&self) -> Result<MobileSettings> {
        MobileSettings::load(&self.store)
    }

    pub fn save_settings(&self, settings: MobileSettings) -> Result<()> {
        settings.save(&self.store)
    }

    // ---- account & sync -----------------------------------------------

    /// Receive sync status / change notifications. Replace with `None` to
    /// stop.
    pub fn set_sync_listener(&self, listener: Option<Arc<dyn SyncListener>>) {
        self.account.set_listener(listener);
    }

    /// Name this installation registers under (shown in the devices list).
    pub fn set_device_name(&self, name: String) {
        self.account.set_device_name(name);
    }

    pub fn account_status(&self) -> Result<AccountStatus> {
        RUNTIME.block_on(self.account.status())
    }

    /// Restore a persisted session and start syncing. Returns the account
    /// even when the server is unreachable (the engine reconnects).
    pub fn account_resume(&self) -> Result<Option<AccountCard>> {
        RUNTIME.block_on(self.account.resume())
    }

    pub fn account_login(&self, form: LoginForm) -> Result<LoginOutcome> {
        RUNTIME.block_on(self.account.login(form))
    }

    pub fn account_register(&self, form: RegisterForm) -> Result<Registered> {
        RUNTIME.block_on(self.account.register(form))
    }

    /// Second factor for a pending sign-in.
    pub fn account_mfa(&self, method: MfaMethod, code: String) -> Result<LoginOutcome> {
        RUNTIME.block_on(self.account.mfa(method, code))
    }

    pub fn account_mfa_email_send(&self) -> Result<()> {
        RUNTIME.block_on(self.account.mfa_email_send())
    }

    /// Code from the device-approval email for a pending sign-in.
    pub fn account_approve_device(&self, code: String) -> Result<LoginOutcome> {
        RUNTIME.block_on(self.account.approve_device(code))
    }

    pub fn account_resend_device_code(&self) -> Result<()> {
        RUNTIME.block_on(self.account.resend_device_code())
    }

    pub fn account_cancel_login(&self) -> Result<()> {
        RUNTIME.block_on(self.account.cancel_login())
    }

    /// Revoke this device on the server and forget the account, synced
    /// vaults and keys locally. The local vault stays.
    pub fn account_sign_out(&self) -> Result<()> {
        RUNTIME.block_on(self.account.sign_out())
    }

    pub fn sync_now(&self) -> Result<SyncStatus> {
        RUNTIME.block_on(self.account.sync_now())
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.account.sync_status()
    }

    pub fn account_devices(&self) -> Result<Vec<DeviceCard>> {
        RUNTIME.block_on(self.account.devices())
    }

    pub fn account_revoke_device(&self, id: String) -> Result<()> {
        RUNTIME.block_on(self.account.revoke_device(id))
    }

    // ---- sessions -----------------------------------------------------

    /// Open a terminal to a saved host. Returns at once; progress, prompts
    /// and output arrive on `listener`.
    pub fn connect_host(
        &self,
        host_id: String,
        options: TerminalOptions,
        listener: Arc<dyn SessionListener>,
    ) -> Result<Arc<SshSession>> {
        let (resolved, target) = self.ssh_host(&host_id)?;
        Ok(SshSession::launch(
            RUNTIME.handle().clone(),
            Launch {
                store: self.store.clone(),
                target,
                resolved: Some(resolved),
                settings: MobileSettings::load(&self.store)?,
                options,
                listener,
            },
        ))
    }

    /// Open a terminal to an ad-hoc target (`user@host:port`); nothing is
    /// saved except the connection history and any trusted host key.
    pub fn connect_quick(
        &self,
        target: QuickTarget,
        options: TerminalOptions,
        listener: Arc<dyn SessionListener>,
    ) -> Result<Arc<SshSession>> {
        Ok(SshSession::launch(
            RUNTIME.handle().clone(),
            Launch {
                store: self.store.clone(),
                target: quick_target(&target)?,
                resolved: None,
                settings: MobileSettings::load(&self.store)?,
                options,
                listener,
            },
        ))
    }

    /// Open SFTP to a saved host. Returns at once; state, prompts and
    /// transfers arrive on `listener`.
    pub fn sftp_host(
        &self,
        host_id: String,
        listener: Arc<dyn SftpListener>,
    ) -> Result<Arc<SftpSession>> {
        let (resolved, target) = self.ssh_host(&host_id)?;
        Ok(SftpSession::launch(
            RUNTIME.handle().clone(),
            SftpLaunch {
                store: self.store.clone(),
                target,
                resolved: Some(resolved),
                settings: MobileSettings::load(&self.store)?,
                listener,
            },
        ))
    }

    /// Open SFTP to an ad-hoc target.
    pub fn sftp_quick(
        &self,
        target: QuickTarget,
        listener: Arc<dyn SftpListener>,
    ) -> Result<Arc<SftpSession>> {
        Ok(SftpSession::launch(
            RUNTIME.handle().clone(),
            SftpLaunch {
                store: self.store.clone(),
                target: quick_target(&target)?,
                resolved: None,
                settings: MobileSettings::load(&self.store)?,
                listener,
            },
        ))
    }

    // ── port forwarding ──

    /// Stored forwarding rules of one vault (all when `None`), labelled first.
    pub fn pf_rules(&self, vault_id: Option<String>) -> Result<Vec<PfRuleItem>> {
        forward::rules(&self.store, &vault_id)
    }

    pub fn pf_rule(&self, id: String) -> Result<PfRuleItem> {
        forward::rule(&self.store, parse_id(&id)?)
    }

    /// Create (`draft.id == None`) or update a rule after validating it.
    pub fn save_pf_rule(&self, draft: PfRuleDraft) -> Result<PfRuleItem> {
        forward::save(&self.store, &draft)
    }

    pub fn duplicate_pf_rule(&self, id: String) -> Result<PfRuleItem> {
        forward::duplicate(&self.store, parse_id(&id)?)
    }

    /// Remove a rule; the caller stops its tunnel first.
    pub fn delete_pf_rule(&self, id: String) -> Result<()> {
        forward::delete(&self.store, parse_id(&id)?)
    }

    /// Start a rule's tunnel. Returns at once; state and prompts arrive on
    /// `listener`. Fails synchronously only when the rule or its host is gone.
    pub fn start_pf(
        &self,
        rule_id: String,
        listener: Arc<dyn TunnelListener>,
    ) -> Result<Arc<PfTunnel>> {
        let rule = forward::resolve_for_tunnel(&self.store, parse_id(&rule_id)?)?;
        Ok(PfTunnel::launch(
            RUNTIME.handle().clone(),
            TunnelLaunch {
                store: self.store.clone(),
                rule,
                settings: MobileSettings::load(&self.store)?,
                listener,
            },
        ))
    }
}

impl TermosoApp {
    fn ssh_host(&self, host_id: &str) -> Result<(ResolvedHost, SshTarget)> {
        let resolved = self.store.resolve_host(parse_id(host_id)?)?;
        if resolved.protocol() != "ssh" {
            return Err(MobileError::invalid(format!(
                "{} hosts are not supported on mobile yet",
                resolved.protocol()
            )));
        }
        let target = SshTarget {
            host: resolved.host.data.address.clone(),
            port: resolved.port(),
            username: resolved.username(),
        };
        Ok((resolved, target))
    }
}

fn quick_target(target: &QuickTarget) -> Result<SshTarget> {
    if target.host.trim().is_empty() {
        return Err(MobileError::invalid("host is empty"));
    }
    Ok(SshTarget {
        host: target.host.trim().to_string(),
        port: target.port,
        username: if target.username.trim().is_empty() {
            "root".into()
        } else {
            target.username.trim().to_string()
        },
    })
}

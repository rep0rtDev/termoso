//! Process-wide application state owned by Rust.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use termoso_core::secrets::{self, MasterKeySource};
use termoso_core::store::Store;
use termoso_core::sync::ConflictPolicy;

use crate::account::AccountRuntime;
use crate::error::{DesktopError, Result};
use crate::forwarding::Forwards;
use crate::prompts::PromptBroker;
use crate::sessions::Sessions;
use crate::sftp::SftpSessions;

pub const PROFILE_ENV: &str = "TERMOSO_PROFILE_DIR";
pub const PROFILE_NAME: &str = "default";
const DB_FILE: &str = "vault.db";
const LOGS_DIR: &str = "logs";
const SETTINGS_META: &str = "desktop.settings";

pub struct AppState {
    pub profile_dir: PathBuf,
    pub master_source: std::sync::Mutex<MasterKeySource>,
    pub store: Arc<Store>,
    pub sessions: Sessions,
    pub sftp: SftpSessions,
    pub forwards: Forwards,
    pub prompts: PromptBroker,
    pub account: AccountRuntime,
}

impl AppState {
    /// Open (or create) the profile: master key from the OS keychain, falling
    /// back to an owner-only file, then the encrypted local store.
    pub fn open() -> Result<Self> {
        let profile_dir = std::env::var_os(PROFILE_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| secrets::default_profile_dir(PROFILE_NAME));
        std::fs::create_dir_all(&profile_dir)?;
        std::fs::create_dir_all(profile_dir.join(LOGS_DIR))?;
        let master = secrets::load_or_create(&profile_dir, PROFILE_NAME, true)?;
        let store = Store::open(&profile_dir.join(DB_FILE), master.key)?;
        Ok(Self {
            profile_dir,
            master_source: std::sync::Mutex::new(master.source),
            store: Arc::new(store),
            sessions: Sessions::default(),
            sftp: SftpSessions::default(),
            forwards: Forwards::default(),
            prompts: PromptBroker::default(),
            account: AccountRuntime::default(),
        })
    }

    pub fn master_source(&self) -> MasterKeySource {
        *self.master_source.lock().expect("master source poisoned")
    }

    /// Move a file-based master key into the OS keychain.
    pub fn migrate_master_key(&self) -> Result<MasterKeySource> {
        if secrets::migrate_file_to_keychain(&self.profile_dir, PROFILE_NAME)? {
            *self.master_source.lock().expect("master source poisoned") = MasterKeySource::Keychain;
        }
        Ok(self.master_source())
    }

    /// Where encrypted session recordings live.
    pub fn logs_dir(&self) -> PathBuf {
        self.profile_dir.join(LOGS_DIR)
    }

    pub fn settings(&self) -> Result<Settings> {
        Ok(match self.store.meta(SETTINGS_META)? {
            Some(raw) => serde_json::from_str(&raw)?,
            None => Settings::default(),
        })
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        self.store
            .set_meta(SETTINGS_META, &serde_json::to_string(settings)?)?;
        Ok(())
    }
}

/// UI preferences. Stored in the (encrypted-at-rest) local database, never
/// leaves the device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// `dark` | `light` | `system`.
    pub theme: String,
    /// `grid` | `list`.
    pub hosts_view: String,
    pub terminal_font_size: u16,
    pub terminal_font_family: String,
    /// Line height multiplier (1.0 = font's natural height).
    pub terminal_line_height: f32,
    /// Colour scheme id from the client's theme registry; `auto` follows
    /// `theme` with the Termoso Dark / Light schemes.
    pub terminal_theme: String,
    pub cursor_blink: bool,
    /// `block` | `underline` | `bar`.
    pub cursor_style: String,
    pub scrollback: u32,
    pub copy_on_select: bool,
    pub paste_on_right_click: bool,
    pub confirm_close_tab: bool,
    pub confirm_paste_multiline: bool,
    pub autocomplete: bool,
    pub terminal_bell: bool,
    pub keep_alive_seconds: u32,
    /// Probe a host's OS after the first successful connection to pick its icon.
    pub detect_os: bool,
    /// Offer the hybrid ML-KEM-768 + X25519 key exchange (servers without it
    /// fall back to classical algorithms).
    pub post_quantum_kex: bool,
    /// Record terminal output of every session into the encrypted log store.
    pub record_sessions: bool,
    /// Delete local recordings older than this many days (0 = keep).
    pub log_retention_days: u32,
    /// Start `auto_start` forwarding rules when the app launches.
    pub autostart_forwarding: bool,
    /// `newest_wins` | `local_wins` | `server_wins`.
    pub sync_conflict: String,
    /// Background sync period in seconds (0 = manual + realtime only).
    pub sync_interval_seconds: u32,
    /// Upload finished recordings to the account server.
    pub upload_logs: bool,
    /// `manual` (never contacts the feed unless asked) | `startup`.
    pub update_check: String,
    /// Release feed URL; empty = project default. Point it at your own server
    /// to keep updates fully self-hosted.
    pub update_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            hosts_view: "grid".into(),
            terminal_font_size: 13,
            terminal_font_family: "JetBrains Mono".into(),
            terminal_line_height: 1.0,
            terminal_theme: "auto".into(),
            cursor_blink: true,
            cursor_style: "block".into(),
            scrollback: 10_000,
            copy_on_select: false,
            paste_on_right_click: true,
            confirm_close_tab: true,
            confirm_paste_multiline: true,
            autocomplete: true,
            terminal_bell: false,
            keep_alive_seconds: 30,
            detect_os: true,
            post_quantum_kex: true,
            record_sessions: false,
            log_retention_days: 0,
            autostart_forwarding: true,
            sync_conflict: "newest_wins".into(),
            sync_interval_seconds: 300,
            upload_logs: false,
            update_check: "manual".into(),
            update_url: String::new(),
        }
    }
}

impl Settings {
    fn validate(&self) -> Result<()> {
        if !matches!(self.theme.as_str(), "dark" | "light" | "system") {
            return Err(DesktopError::invalid("theme must be dark, light or system"));
        }
        if !matches!(self.hosts_view.as_str(), "grid" | "list") {
            return Err(DesktopError::invalid("hostsView must be grid or list"));
        }
        if !matches!(self.cursor_style.as_str(), "block" | "underline" | "bar") {
            return Err(DesktopError::invalid(
                "cursorStyle must be block, underline or bar",
            ));
        }
        if !(6..=72).contains(&self.terminal_font_size) {
            return Err(DesktopError::invalid("terminalFontSize out of range"));
        }
        if !(0.8..=2.0).contains(&self.terminal_line_height) {
            return Err(DesktopError::invalid(
                "terminalLineHeight must be between 0.8 and 2.0",
            ));
        }
        if self.terminal_theme.is_empty() || self.terminal_theme.len() > 64 {
            return Err(DesktopError::invalid("terminalTheme must be 1-64 chars"));
        }
        if self.scrollback > 1_000_000 {
            return Err(DesktopError::invalid("scrollback too large"));
        }
        if self.keep_alive_seconds > 3600 {
            return Err(DesktopError::invalid("keepAliveSeconds too large"));
        }
        if self.log_retention_days > 3650 {
            return Err(DesktopError::invalid("logRetentionDays too large"));
        }
        self.conflict_policy()?;
        if self.sync_interval_seconds != 0 && !(30..=86_400).contains(&self.sync_interval_seconds) {
            return Err(DesktopError::invalid(
                "syncIntervalSeconds must be 0 or between 30 and 86400",
            ));
        }
        if !matches!(self.update_check.as_str(), "manual" | "startup") {
            return Err(DesktopError::invalid(
                "updateCheck must be manual or startup",
            ));
        }
        crate::update::feed_url(&self.update_url)?;
        Ok(())
    }

    pub fn conflict_policy(&self) -> Result<ConflictPolicy> {
        match self.sync_conflict.as_str() {
            "newest_wins" => Ok(ConflictPolicy::NewestWins),
            "local_wins" => Ok(ConflictPolicy::LocalWins),
            "server_wins" => Ok(ConflictPolicy::ServerWins),
            _ => Err(DesktopError::invalid(
                "syncConflict must be newest_wins, local_wins or server_wins",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_old_blobs_still_load() {
        Settings::default().validate().unwrap();
        // A blob written by an older build (fewer fields) must still parse.
        let s: Settings = serde_json::from_str(r#"{"theme":"light","scrollback":500}"#).unwrap();
        assert_eq!(s.theme, "light");
        assert_eq!(s.scrollback, 500);
        assert_eq!(s.cursor_style, "block");
        assert!(!s.record_sessions);
        s.validate().unwrap();
    }

    #[test]
    fn rejects_bad_values() {
        let s = Settings {
            cursor_style: "blink".into(),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        let s = Settings {
            sync_conflict: "coin_flip".into(),
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        let mut s = Settings {
            sync_interval_seconds: 5,
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        s.sync_interval_seconds = 0;
        assert!(s.validate().is_ok());
        let s = Settings {
            log_retention_days: 100_000,
            ..Settings::default()
        };
        assert!(s.validate().is_err());
    }
}

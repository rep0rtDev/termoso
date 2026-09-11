//! Process-wide application state owned by Rust.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use termoso_core::secrets::{self, MasterKeySource};
use termoso_core::store::Store;

use crate::error::{DesktopError, Result};
use crate::prompts::PromptBroker;
use crate::sessions::Sessions;

pub const PROFILE_ENV: &str = "TERMOSO_PROFILE_DIR";
const DB_FILE: &str = "vault.db";
const SETTINGS_META: &str = "desktop.settings";

pub struct AppState {
    pub profile_dir: PathBuf,
    pub master_source: MasterKeySource,
    pub store: Arc<Store>,
    pub sessions: Sessions,
    pub prompts: PromptBroker,
}

impl AppState {
    /// Open (or create) the profile: master key from the OS keychain, falling
    /// back to an owner-only file, then the encrypted local store.
    pub fn open() -> Result<Self> {
        let profile_dir = std::env::var_os(PROFILE_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| secrets::default_profile_dir("default"));
        std::fs::create_dir_all(&profile_dir)?;
        let master = secrets::load_or_create(&profile_dir, "default", true)?;
        let store = Store::open(&profile_dir.join(DB_FILE), master.key)?;
        Ok(Self {
            profile_dir,
            master_source: master.source,
            store: Arc::new(store),
            sessions: Sessions::default(),
            prompts: PromptBroker::default(),
        })
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
    pub cursor_blink: bool,
    pub scrollback: u32,
    pub copy_on_select: bool,
    pub paste_on_right_click: bool,
    pub confirm_close_tab: bool,
    pub confirm_paste_multiline: bool,
    pub autocomplete: bool,
    pub keep_alive_seconds: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            hosts_view: "grid".into(),
            terminal_font_size: 13,
            terminal_font_family: "JetBrains Mono Variable".into(),
            cursor_blink: true,
            scrollback: 10_000,
            copy_on_select: false,
            paste_on_right_click: true,
            confirm_close_tab: true,
            confirm_paste_multiline: true,
            autocomplete: true,
            keep_alive_seconds: 30,
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
        if !(6..=72).contains(&self.terminal_font_size) {
            return Err(DesktopError::invalid("terminalFontSize out of range"));
        }
        if self.scrollback > 1_000_000 {
            return Err(DesktopError::invalid("scrollback too large"));
        }
        Ok(())
    }
}

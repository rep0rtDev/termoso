//! Mobile settings, stored as JSON in the encrypted store's metadata so they
//! travel with the vault backup and never sit in plain `SharedPreferences`.

use serde::{Deserialize, Serialize};
use termoso_core::store::Store;

use crate::error::Result;

const SETTINGS_META: &str = "mobile.settings";

fn default_true() -> bool {
    true
}
fn default_font_size() -> u32 {
    14
}
fn default_scrollback() -> u32 {
    5000
}
fn default_keepalive() -> u32 {
    30
}
fn default_term() -> String {
    "xterm-256color".into()
}
fn default_theme() -> String {
    crate::themes::TERMOSO_DARK.into()
}
fn default_cursor() -> String {
    "block".into()
}
fn default_view() -> String {
    "list".into()
}
fn default_app_theme() -> String {
    "system".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
#[serde(default, rename_all = "camelCase")]
pub struct MobileSettings {
    /// `system` | `dark` | `light`.
    #[serde(default = "default_app_theme")]
    pub app_theme: String,
    /// `list` | `grid`.
    #[serde(default = "default_view")]
    pub hosts_view: String,
    #[serde(default = "default_theme")]
    pub terminal_theme: String,
    #[serde(default = "default_font_size")]
    pub terminal_font_size: u32,
    pub terminal_font_family: String,
    /// `block` | `underline` | `beam`.
    #[serde(default = "default_cursor")]
    pub cursor_style: String,
    #[serde(default = "default_true")]
    pub cursor_blink: bool,
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: u32,
    #[serde(default = "default_term")]
    pub term_type: String,
    #[serde(default = "default_true")]
    pub terminal_bell: bool,
    #[serde(default = "default_true")]
    pub haptic_feedback: bool,
    /// Keep the screen on while a terminal is in front.
    #[serde(default = "default_true")]
    pub keep_screen_on: bool,
    /// Seconds between SSH keepalives; 0 disables.
    #[serde(default = "default_keepalive")]
    pub keep_alive_seconds: u32,
    #[serde(default = "default_true")]
    pub post_quantum_kex: bool,
    #[serde(default = "default_true")]
    pub detect_os: bool,
    /// Ask for the device credential (biometric / PIN) when the app returns
    /// to the foreground.
    pub lock_on_background: bool,
    /// Seconds in background before the lock kicks in.
    pub lock_after_seconds: u32,
    /// Sync identities, keys and certificates of the Personal vault. Off
    /// keeps them on this phone (hosts etc. still sync); they are removed
    /// together with the account on sign-out.
    #[serde(default = "default_true")]
    pub sync_credentials: bool,
    /// Record terminal output of every session into the encrypted log
    /// store (team vaults with session logging on record regardless).
    #[serde(default)]
    pub record_sessions: bool,
    /// Onboarding shown.
    pub welcome_seen: bool,
    /// Vault shown in the Vaults tab when the app was last used.
    #[serde(default)]
    pub selected_vault_id: Option<String>,
}

impl Default for MobileSettings {
    fn default() -> Self {
        Self {
            app_theme: default_app_theme(),
            hosts_view: default_view(),
            terminal_theme: default_theme(),
            terminal_font_size: default_font_size(),
            terminal_font_family: String::new(),
            cursor_style: default_cursor(),
            cursor_blink: true,
            scrollback_lines: default_scrollback(),
            term_type: default_term(),
            terminal_bell: true,
            haptic_feedback: true,
            keep_screen_on: true,
            keep_alive_seconds: default_keepalive(),
            post_quantum_kex: true,
            detect_os: true,
            lock_on_background: false,
            lock_after_seconds: 0,
            sync_credentials: true,
            record_sessions: false,
            welcome_seen: false,
            selected_vault_id: None,
        }
    }
}

impl MobileSettings {
    pub(crate) fn load(store: &Store) -> Result<Self> {
        Ok(match store.meta(SETTINGS_META)? {
            Some(json) => serde_json::from_str(&json).unwrap_or_default(),
            None => Self::default(),
        })
    }

    pub(crate) fn save(&self, store: &Store) -> Result<()> {
        let mut s = self.clone();
        s.scrollback_lines = s.scrollback_lines.clamp(0, 100_000);
        s.terminal_font_size = s.terminal_font_size.clamp(6, 40);
        if s.term_type.trim().is_empty() {
            s.term_type = default_term();
        }
        store.set_meta(SETTINGS_META, &serde_json::to_string(&s)?)?;
        Ok(())
    }
}

//! Termoso mobile façade: the API the Android app talks to, exported through
//! UniFFI. The Rust side owns storage, encryption, SSH and terminal
//! emulation; Kotlin owns the Keystore-wrapped master key, the UI and the
//! foreground service.
//!
//! Secrets policy: list/card records never carry passwords, passphrases or
//! private keys. The only calls that return private material are
//! [`TermosoApp::export_private_key`] (explicit user action) and prompt
//! answers travelling *into* Rust.

#![forbid(unsafe_code)]

uniffi::setup_scaffolding!("termoso");

mod account;
mod app;
mod connect;
mod dto;
mod error;
mod forward;
mod keys;
mod session;
mod settings;
mod sftp;
mod snippets;
mod terminal;
mod themes;

pub use account::*;
pub use app::*;
pub use connect::{HostKeyChoice, InteractiveQuestionInfo, PromptAnswer, PromptRequest};
pub use dto::*;
pub use error::{MobileError, Result};
pub use forward::{
    PfKind, PfRuleDraft, PfRuleItem, PfTunnel, TunnelListener, TunnelState, TunnelStats,
};
pub use keys::{KeyMods, SpecialKey, control_code, encode_key, encode_text};
pub use session::*;
pub use settings::MobileSettings;
pub use sftp::*;
pub use snippets::{SnippetDraft, SnippetItem, SnippetPackageItem, SnippetRun};
pub use terminal::{CELL_BYTES, CursorStyle, GridFrame, GridSnapshot, TerminalPalette, flag};
pub use themes::{TERMOSO_DARK, TERMOSO_LIGHT, TerminalTheme, terminal_theme, terminal_themes};

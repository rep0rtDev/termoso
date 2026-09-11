//! Termoso client core.
//!
//! Everything a Termoso client does that is not pixels lives here so the
//! desktop (Tauri) and mobile (UniFFI) shells stay thin:
//!
//! * [`store`] – encrypted local database (SQLite). Works fully offline with a
//!   *local vault*; an account only adds synchronisation.
//! * [`secrets`] – the single local master key, kept in the OS keychain.
//! * [`api`] – typed client for the Termoso server (OPAQUE login, vaults, sync,
//!   history, logs, realtime notifications).
//! * [`sync`] – bidirectional sync engine between [`store`] and [`api`] with
//!   optimistic concurrency and deterministic conflict resolution.
//! * [`ssh`] – SSH transport (russh): host-key trust, password / key / agent /
//!   keyboard-interactive auth, interactive PTY, exec, port forwarding, jump
//!   hosts.
//! * [`sftp`] – SFTP over an SSH session with progress reporting.
//! * [`terminal`] – protocol-agnostic interactive session (SSH shell, local
//!   PTY, Telnet) that the UI drives with bytes and resize events.
//! * [`keys`] – SSH key generation, import, export and fingerprints.
//! * [`agent`] – in-process SSH agent serving the keys in the vault.
//! * [`osdetect`] – figure out what OS a host runs (for its icon).
//!
//! Nothing in this crate phones home: the only network peers are the servers
//! the user connects to and the Termoso server they configured.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod account;
pub mod agent;
pub mod api;
pub mod error;
pub mod forward;
pub mod hostkey;
pub mod keys;
pub mod model;
pub mod osdetect;
pub mod secrets;
pub mod sftp;
pub mod ssh;
pub mod store;
pub mod sync;
pub mod telnet;
pub mod terminal;

#[cfg(feature = "local-pty")]
pub mod pty;

pub use error::CoreError;
pub use termoso_crypto;
pub use termoso_proto;

/// Client version reported to servers (`app_version`).
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

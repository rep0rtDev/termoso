//! Client façades shared by the Termoso shells (Tauri desktop, Android).
//!
//! Everything here is pure `Store` logic: the UI edits flat forms
//! ([`hosts::HostForm`], [`keychain::GenerateForm`], …) and Rust maps them to
//! the sync entities, resolves group inheritance and keeps secrets from
//! leaking into list DTOs. Nothing in this crate touches the network or a
//! window system.

#![forbid(unsafe_code)]

pub mod error;
pub mod hosts;
pub mod keychain;

pub use error::{ClientError, Result};

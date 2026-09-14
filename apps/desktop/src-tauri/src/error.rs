//! Error shape handed to the webview: `{ kind, message }` — the shared
//! [`termoso_client::ClientError`].

pub use termoso_client::ClientError as DesktopError;

pub type Result<T> = std::result::Result<T, DesktopError>;

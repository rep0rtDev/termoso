//! Multiplayer on the phone: share an open terminal with teammates, or watch
//! (and, when allowed, drive) a terminal somebody else shares.
//!
//! All of the protocol lives in `termoso_core::live` — link secret, key
//! derivation, frame sealing, relay socket. This module ties it to
//! [`SshSession`]: a **host** session gets a tap that mirrors its output into
//! the share, a **viewer** session is an [`SshSession`] whose "remote" is the
//! relay stream. Kotlin only ever sees the opaque link the host hands out and
//! presence/permission records.

use std::sync::{Arc, Mutex};

use termoso_core::live::{HostShare, LiveEvent, LiveLink, Publisher};
use termoso_core::terminal::TermSize;
use termoso_proto::live::LiveParticipant;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::dto::parse_id;
use crate::error::{MobileError, Result};

/// Somebody connected to a shared terminal.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LiveParticipantCard {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    /// Profile picture tag, fetched through `TermosoApp::user_avatar`.
    pub avatar: Option<String>,
    /// The person sharing.
    pub is_host: bool,
    /// May type into the terminal (always true for the host).
    pub can_write: bool,
    /// This device.
    pub me: bool,
}

/// Why a share or a view ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum LiveEndReason {
    /// The host stopped multiplayer.
    Stopped,
    /// The relay connection dropped.
    Disconnected,
    /// The relay reported an error.
    Error,
}

/// Presence and permission changes of one share / view. Invoked from Rust
/// worker threads; keep it quick.
#[uniffi::export(with_foreign)]
pub trait LiveListener: Send + Sync {
    /// Everyone connected, host first.
    fn on_participants(&self, participants: Vec<LiveParticipantCard>);
    /// Viewer only: the host granted / revoked remote control.
    fn on_control(&self, can_write: bool);
    /// The share (host) or the view (viewer) is over.
    fn on_ended(&self, reason: LiveEndReason, message: String);
}

pub(crate) fn cards(list: &[LiveParticipant], me: Uuid) -> Vec<LiveParticipantCard> {
    let mut v: Vec<LiveParticipantCard> = list
        .iter()
        .map(|p| LiveParticipantCard {
            user_id: p.user_id.to_string(),
            email: p.email.clone(),
            display_name: p.display_name.clone(),
            avatar: p.avatar.clone(),
            is_host: p.is_host,
            can_write: p.can_write,
            me: p.user_id == me,
        })
        .collect();
    v.sort_by_key(|p| (!p.is_host, !p.me, p.email.clone()));
    v
}

fn end_reason(reason: &str) -> LiveEndReason {
    match reason {
        "stopped" => LiveEndReason::Stopped,
        "disconnected" => LiveEndReason::Disconnected,
        _ => LiveEndReason::Error,
    }
}

/// Is `s` a multiplayer join link (`https://<server>/join/…#…` or `termoso://join/…`)?
#[uniffi::export]
pub fn is_live_link(s: String) -> bool {
    LiveLink::looks_like(&s)
}

/// Host side of one shared terminal, owned by the [`SshSession`] it mirrors
/// until it is stopped or the tab closes.
pub(crate) struct ShareState {
    share: Mutex<Option<HostShare>>,
    pub(crate) publisher: Publisher,
    me: Uuid,
    participants: Mutex<Vec<LiveParticipant>>,
}

impl ShareState {
    pub(crate) fn resized(&self, size: TermSize) {
        if let Some(s) = self.share.lock().expect("share poisoned").as_ref() {
            s.resized(size);
        }
    }

    pub(crate) fn retitled(&self, title: String) {
        if let Some(s) = self.share.lock().expect("share poisoned").as_ref() {
            s.retitled(title);
        }
    }

    fn take(&self) -> Option<HostShare> {
        self.share.lock().expect("share poisoned").take()
    }

    /// End the share because the terminal went away (viewers get `stopped`).
    pub(crate) fn shutdown(&self, runtime: &tokio::runtime::Handle) {
        if let Some(share) = self.take() {
            runtime.spawn(share.stop());
        }
    }
}

/// A terminal this device is sharing. Lives as long as the session it
/// mirrors: closing that terminal ends the share, dropping this handle does
/// not.
#[derive(uniffi::Object)]
pub struct LiveShare {
    live_id: Uuid,
    link: String,
    state: Arc<ShareState>,
    runtime: tokio::runtime::Handle,
}

impl LiveShare {
    /// Wire a started [`HostShare`] to a session; `detach` is called when the
    /// share ends for any reason so the session can drop its tap.
    pub(crate) fn attach(
        runtime: tokio::runtime::Handle,
        share: HostShare,
        rx: mpsc::Receiver<LiveEvent>,
        listener: Arc<dyn LiveListener>,
        detach: Box<dyn Fn() + Send + Sync>,
    ) -> (Arc<Self>, Arc<ShareState>) {
        let state = Arc::new(ShareState {
            publisher: share.publisher(),
            me: share.user_id(),
            participants: Mutex::new(Vec::new()),
            share: Mutex::new(None),
        });
        let this = Arc::new(Self {
            live_id: share.session_id(),
            link: share.link().to_string(),
            state: state.clone(),
            runtime: runtime.clone(),
        });
        *state.share.lock().expect("share poisoned") = Some(share);
        runtime.spawn(forward_host(rx, state.clone(), listener, detach));
        (this, state)
    }
}

async fn forward_host(
    mut rx: mpsc::Receiver<LiveEvent>,
    state: Arc<ShareState>,
    listener: Arc<dyn LiveListener>,
    detach: Box<dyn Fn() + Send + Sync>,
) {
    while let Some(ev) = rx.recv().await {
        match ev {
            LiveEvent::Participants { participants } => {
                let cards = cards(&participants, state.me);
                *state.participants.lock().expect("participants poisoned") = participants;
                listener.on_participants(cards);
            }
            LiveEvent::Ended { reason, message } => {
                drop(state.take());
                detach();
                listener.on_ended(end_reason(&reason), message);
                return;
            }
            LiveEvent::Control { .. } | LiveEvent::Resize { .. } | LiveEvent::Title { .. } => {}
        }
    }
}

#[uniffi::export]
impl LiveShare {
    /// Relay session id.
    pub fn live_id(&self) -> String {
        self.live_id.to_string()
    }

    /// The `https://<server>/join/…#<secret>` link to hand out. Contains the secret: show it
    /// and copy it, never log it.
    pub fn link(&self) -> String {
        self.link.clone()
    }

    /// Everyone connected right now, host first.
    pub fn participants(&self) -> Vec<LiveParticipantCard> {
        cards(
            &self
                .state
                .participants
                .lock()
                .expect("participants poisoned"),
            self.state.me,
        )
    }

    /// Let a viewer type into the terminal, or take that back.
    pub fn set_control(&self, user_id: String, enabled: bool) -> Result<()> {
        let user_id = parse_id(&user_id)?;
        let guard = self.state.share.lock().expect("share poisoned");
        let share = guard
            .as_ref()
            .ok_or_else(|| MobileError::not_found("multiplayer session"))?;
        share.set_control(user_id, enabled)?;
        Ok(())
    }

    /// Whether the share is still running.
    pub fn is_active(&self) -> bool {
        self.state.share.lock().expect("share poisoned").is_some()
    }

    /// Stop sharing: the server tells every viewer, the terminal keeps
    /// running. Returns once the relay connection is closed.
    pub fn stop(&self) {
        if let Some(share) = self.state.take() {
            self.runtime.block_on(share.stop());
        }
    }
}

/// Viewer-side bookkeeping kept on the [`SshSession`].
pub(crate) struct ViewState {
    pub(crate) me: Uuid,
    pub(crate) participants: Mutex<Vec<LiveParticipant>>,
    pub(crate) can_write: std::sync::atomic::AtomicBool,
}

impl ViewState {
    pub(crate) fn cards(&self) -> Vec<LiveParticipantCard> {
        cards(
            &self.participants.lock().expect("participants poisoned"),
            self.me,
        )
    }
}

/// Forward relay events of a view to the listener and keep the state current.
/// `resize` is called with the host's geometry so the emulator follows it.
pub(crate) async fn forward_viewer(
    mut rx: mpsc::Receiver<LiveEvent>,
    state: Arc<ViewState>,
    listener: Arc<dyn LiveListener>,
    resize: Box<dyn Fn(u16, u16) + Send + Sync>,
    title: Box<dyn Fn(String) + Send + Sync>,
) {
    while let Some(ev) = rx.recv().await {
        match ev {
            LiveEvent::Participants { participants } => {
                let cards = cards(&participants, state.me);
                *state.participants.lock().expect("participants poisoned") = participants;
                listener.on_participants(cards);
            }
            LiveEvent::Control { can_write } => {
                state
                    .can_write
                    .store(can_write, std::sync::atomic::Ordering::Relaxed);
                listener.on_control(can_write);
            }
            LiveEvent::Resize { cols, rows } => resize(cols, rows),
            LiveEvent::Title { title: t } => title(t),
            LiveEvent::Ended { reason, message } => {
                listener.on_ended(end_reason(&reason), message);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_put_host_then_me_first() {
        let me = Uuid::new_v4();
        let host = Uuid::new_v4();
        let list = vec![
            LiveParticipant {
                user_id: Uuid::new_v4(),
                email: "a@x".into(),
                display_name: None,
                avatar: None,
                is_host: false,
                can_write: false,
            },
            LiveParticipant {
                user_id: me,
                email: "me@x".into(),
                display_name: Some("Me".into()),
                avatar: None,
                is_host: false,
                can_write: true,
            },
            LiveParticipant {
                user_id: host,
                email: "host@x".into(),
                display_name: None,
                avatar: None,
                is_host: true,
                can_write: true,
            },
        ];
        let cards = cards(&list, me);
        assert!(cards[0].is_host);
        assert!(cards[1].me);
        assert_eq!(cards[2].email, "a@x");
    }

    #[test]
    fn end_reasons_map() {
        assert_eq!(end_reason("stopped"), LiveEndReason::Stopped);
        assert_eq!(end_reason("disconnected"), LiveEndReason::Disconnected);
        assert_eq!(end_reason("weird"), LiveEndReason::Error);
    }
}

//! The façade as Kotlin sees it: open a profile, edit hosts / keys /
//! settings, then drive an SSH session against an in-process `russh`
//! server through the listener callbacks.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use russh::keys::ssh_key::{Algorithm, PrivateKey};
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId, MethodSet};
use termoso_mobile::{
    HostKeyChoice, IdentityDraft, KeyAlgorithm, KeyGenerateDraft, KeyImportDraft, MobileError,
    PromptAnswer, PromptRequest, QuickTarget, SessionListener, SessionState, SshSession,
    TerminalOptions, TermosoApp, VaultKind, flag, generate_master_key, parse_target,
    profile_exists,
};
use tokio::net::TcpListener;

const USER: &str = "tester";
const PASSWORD: &str = "s3cret";

// ---- tiny SSH server ------------------------------------------------------

#[derive(Clone, Default)]
struct Srv {
    kbd: bool,
}

struct Handler {
    srv: Srv,
    shells: HashMap<ChannelId, ()>,
}

impl russh::server::Server for Srv {
    type Handler = Handler;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Handler {
        Handler {
            srv: self.clone(),
            shells: HashMap::new(),
        }
    }
}

impl Handler {
    fn offered(&self) -> Auth {
        let methods: &[russh::MethodKind] = if self.srv.kbd {
            &[russh::MethodKind::KeyboardInteractive]
        } else {
            &[russh::MethodKind::Password]
        };
        Auth::Reject {
            proceed_with_methods: Some(MethodSet::from(methods)),
            partial_success: false,
        }
    }
}

impl russh::server::Handler for Handler {
    type Error = russh::Error;

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(self.offered())
    }

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if !self.srv.kbd && user == USER && password == PASSWORD {
            Auth::Accept
        } else {
            self.offered()
        })
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<russh::server::Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        match response {
            None => Ok(Auth::Partial {
                name: "Verification".into(),
                instructions: "Answer the prompts".into(),
                prompts: vec![("Code: ".into(), false)].into(),
            }),
            Some(mut r) => {
                let code = r.next().unwrap_or_default();
                Ok(if code.as_ref() == b"424242" {
                    Auth::Accept
                } else {
                    self.offered()
                })
            }
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let _ = channel;
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _cols: u32,
        _rows: u32,
        _pw: u32,
        _ph: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        cols: u32,
        rows: u32,
        _pw: u32,
        _ph: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(
            channel,
            Bytes::from(format!("\r\nresized {cols}x{rows}\r\n")),
        )?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.shells.insert(channel, ());
        session.channel_success(channel)?;
        session.data(
            channel,
            Bytes::from_static(b"\x1b]0;tester@box\x07\x1b[1;32mprompt$\x1b[0m "),
        )?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        cmd: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        if cmd == termoso_core::osdetect::DETECT_COMMAND.as_bytes() {
            session.data(
                channel,
                Bytes::from_static(b"Linux\nPRETTY_NAME=\"Ubuntu 24.04\"\nID=ubuntu\n"),
            )?;
        }
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.shells.contains_key(&channel) {
            if data == b"exit\r" || data == b"exit\n" {
                session.data(channel, Bytes::from_static(b"\r\nbye\r\n"))?;
                session.exit_status_request(channel, 0)?;
                session.eof(channel)?;
                session.close(channel)?;
            } else {
                session.data(channel, Bytes::copy_from_slice(data))?;
            }
        }
        Ok(())
    }
}

async fn start(kbd: bool) -> u16 {
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let mut server = Srv { kbd };
    let config = Arc::new(russh::server::Config {
        auth_rejection_time: Duration::from_millis(0),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        keys: vec![host_key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = server.run_on_socket(config, &listener).await;
    });
    port
}

// ---- listener that records everything ------------------------------------

#[derive(Default)]
struct Recorder {
    states: Mutex<Vec<SessionState>>,
    prompts: Mutex<Vec<(u64, PromptRequest)>>,
    titles: Mutex<Vec<Option<String>>>,
    os: Mutex<Vec<String>>,
    renders: Mutex<u32>,
}

impl SessionListener for Recorder {
    fn on_state(&self, state: SessionState) {
        self.states.lock().unwrap().push(state);
    }
    fn on_render(&self) {
        *self.renders.lock().unwrap() += 1;
    }
    fn on_prompt(&self, prompt_id: u64, request: PromptRequest) {
        self.prompts.lock().unwrap().push((prompt_id, request));
    }
    fn on_title(&self, title: Option<String>) {
        self.titles.lock().unwrap().push(title);
    }
    fn on_bell(&self) {}
    fn on_clipboard(&self, _text: String) {}
    fn on_os_detected(&self, os_name: String) {
        self.os.lock().unwrap().push(os_name);
    }
}

impl Recorder {
    fn wait_prompt(&self, after: usize) -> (u64, PromptRequest) {
        let t = Instant::now();
        loop {
            if let Some(p) = self.prompts.lock().unwrap().get(after) {
                return p.clone();
            }
            assert!(
                t.elapsed() < Duration::from_secs(15),
                "no prompt #{after}; states {:?}",
                self.states.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_state(&self, pred: impl Fn(&SessionState) -> bool) -> SessionState {
        let t = Instant::now();
        loop {
            if let Some(s) = self.states.lock().unwrap().iter().find(|s| pred(s)) {
                return s.clone();
            }
            assert!(
                t.elapsed() < Duration::from_secs(15),
                "state not reached; seen {:?}",
                self.states.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn wait_text(session: &SshSession, needle: &str) -> Vec<String> {
    let t = Instant::now();
    loop {
        let rows = session.visible_text();
        if rows.iter().any(|r| r.contains(needle)) {
            return rows;
        }
        assert!(
            t.elapsed() < Duration::from_secs(15),
            "{needle:?} not on screen: {rows:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn app() -> (Arc<TermosoApp>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let key = generate_master_key();
    assert_eq!(key.len(), 32);
    let app = TermosoApp::open(dir.path().to_string_lossy().into_owned(), key).unwrap();
    (app, dir)
}

fn opts() -> TerminalOptions {
    TerminalOptions {
        cols: 40,
        rows: 6,
        term_type: String::new(),
        palette: None,
    }
}

// ---- tests ----------------------------------------------------------------

#[test]
fn profile_roundtrip_and_wrong_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_string_lossy().into_owned();
    assert!(!profile_exists(path.clone()));
    let key = generate_master_key();
    let app = TermosoApp::open(path.clone(), key.clone()).unwrap();
    let vault = app.local_vault().unwrap();
    assert_eq!(vault.kind, VaultKind::Local);
    let mut d = app.new_host_draft(vault.id.clone(), None).unwrap();
    d.label = "box".into();
    d.address = "10.0.0.1".into();
    app.save_host(d).unwrap();
    drop(app);
    assert!(profile_exists(path.clone()));

    let again = TermosoApp::open(path.clone(), key).unwrap();
    assert_eq!(again.hosts(None).unwrap().len(), 1);
    drop(again);

    assert!(TermosoApp::open(path.clone(), generate_master_key()).is_err());
    assert!(matches!(
        TermosoApp::open(path, vec![1, 2, 3]),
        Err(MobileError::Invalid { .. })
    ));
}

#[test]
fn hosts_crud_without_secrets_in_cards() {
    let (app, _dir) = app();
    let vault = app.local_vault().unwrap().id;
    let group = app
        .save_group(vault.clone(), None, "prod".into(), None)
        .unwrap();

    let mut d = app
        .new_host_draft(vault.clone(), Some(group.id.clone()))
        .unwrap();
    d.label = "web-1".into();
    d.address = "web1.example.org".into();
    d.username = "deploy".into();
    d.port = Some(2222);
    d.password = Some("hunter2".into());
    let web = app.create_tag(vault.clone(), "web".into()).unwrap();
    let eu = app.create_tag(vault.clone(), " eu ".into()).unwrap();
    assert_eq!(
        app.create_tag(vault.clone(), "WEB".into()).unwrap().id,
        web.id
    );
    d.tag_ids = vec![web.id, eu.id];
    let card = app.save_host(d).unwrap();
    assert_eq!(card.label, "web-1");
    assert_eq!(card.port, 2222);
    assert_eq!(card.username, "deploy");
    assert_eq!(card.group_path, vec!["prod".to_string()]);
    assert_eq!(card.tags.len(), 2);
    let json = serde_json::to_string(&serde_json::json!({
        "label": card.label, "address": card.address, "notes": card.notes,
        "username": card.username, "protocol": card.protocol,
    }))
    .unwrap();
    assert!(!json.contains("hunter2"));

    let draft = app.host_draft(card.id.clone()).unwrap();
    assert!(draft.has_password);
    assert_eq!(draft.password, None, "stored password never leaves Rust");

    // Edit keeps the password when the draft leaves it as None.
    let mut edit = draft.clone();
    edit.label = "web-01".into();
    let card2 = app.save_host(edit).unwrap();
    assert_eq!(card2.label, "web-01");
    assert!(app.host_draft(card2.id.clone()).unwrap().has_password);

    // Some("") clears it.
    let mut clear = app.host_draft(card2.id.clone()).unwrap();
    clear.password = Some(String::new());
    app.save_host(clear).unwrap();
    assert!(!app.host_draft(card2.id.clone()).unwrap().has_password);

    assert_eq!(app.hosts(Some(vault.clone())).unwrap().len(), 1);
    assert_eq!(app.tags(None).unwrap().len(), 2);
    let dup = app.duplicate_host(card2.id.clone()).unwrap();
    assert_ne!(dup.id, card2.id);
    app.move_hosts(vec![dup.id.clone()], None).unwrap();
    assert!(app.host(dup.id.clone()).unwrap().group_path.is_empty());
    app.delete_hosts(vec![card2.id, dup.id]).unwrap();
    assert!(app.hosts(None).unwrap().is_empty());
    app.delete_group(group.id).unwrap();
    assert!(app.groups(None).unwrap().is_empty());
    assert!(matches!(
        app.host("not-a-uuid".into()),
        Err(MobileError::Invalid { .. })
    ));
}

#[test]
fn keychain_and_identities() {
    let (app, _dir) = app();
    let vault = app.local_vault().unwrap().id;
    let key = app
        .generate_key(KeyGenerateDraft {
            vault_id: vault.clone(),
            label: "phone".into(),
            algorithm: KeyAlgorithm::Ed25519,
            comment: "termoso@phone".into(),
            passphrase: Some("pp".into()),
            remember_passphrase: false,
        })
        .unwrap();
    assert_eq!(key.key_type, "ssh-ed25519");
    assert!(key.public_key.starts_with("ssh-ed25519 "));
    assert!(key.encrypted && !key.has_passphrase);
    assert!(
        app.public_key(key.id.clone())
            .unwrap()
            .starts_with("ssh-ed25519 ")
    );

    // Only the explicit export returns private material, and it needs the
    // passphrase when it is not remembered.
    assert!(app.export_private_key(key.id.clone(), None, None).is_err());
    let pem = app
        .export_private_key(key.id.clone(), Some("pp".into()), None)
        .unwrap();
    assert!(pem.contains("BEGIN OPENSSH PRIVATE KEY"));

    let preview = app.inspect_private_key(pem.clone()).unwrap();
    assert_eq!(preview.key_type, "ssh-ed25519");
    let imported = app
        .import_key(KeyImportDraft {
            vault_id: vault.clone(),
            label: "imported".into(),
            private_key: pem,
            passphrase: None,
            remember_passphrase: false,
            certificate: None,
        })
        .unwrap();
    assert_eq!(imported.fingerprint, key.fingerprint);

    let ident = app
        .save_identity(IdentityDraft {
            id: None,
            vault_id: vault.clone(),
            label: "deploy".into(),
            username: "deploy".into(),
            password: Some("pw".into()),
            ssh_key_id: Some(key.id.clone()),
        })
        .unwrap();
    assert!(ident.has_password);
    assert_eq!(ident.ssh_key_label.as_deref(), Some("phone"));
    assert_eq!(app.keys(None).unwrap().len(), 2);
    assert_eq!(
        app.keys(None).unwrap()[0].used_by + app.keys(None).unwrap()[1].used_by,
        1
    );

    app.delete_identity(ident.id).unwrap();
    app.delete_key(key.id).unwrap();
    app.delete_key(imported.id).unwrap();
    assert!(app.keys(None).unwrap().is_empty());
    assert!(app.identities(None).unwrap().is_empty());
}

#[test]
fn settings_persist_with_defaults() {
    let (app, _dir) = app();
    let mut s = app.settings().unwrap();
    assert_eq!(s.terminal_font_size, 14);
    assert!(!s.welcome_seen);
    s.welcome_seen = true;
    s.terminal_font_size = 99; // clamped
    s.app_theme = "dark".into();
    app.save_settings(s).unwrap();
    let s = app.settings().unwrap();
    assert!(s.welcome_seen);
    assert_eq!(s.terminal_font_size, 40);
    assert_eq!(s.app_theme, "dark");
}

#[test]
fn parse_targets() {
    let t = parse_target("ssh://deploy@example.org:2200".into()).unwrap();
    assert_eq!(
        (t.username.as_str(), t.host.as_str(), t.port),
        ("deploy", "example.org", 2200)
    );
    let t = parse_target("example.org".into()).unwrap();
    assert_eq!((t.username.as_str(), t.port), ("root", 22));
    let t = parse_target("me@[::1]:23".into()).unwrap();
    assert_eq!((t.host.as_str(), t.port), ("::1", 23));
    assert!(parse_target("  ".into()).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn quick_connect_password_flow() {
    let port = start(false).await;
    let (app, _dir) = app();
    let rec = Arc::new(Recorder::default());
    let session = app
        .connect_quick(
            QuickTarget {
                host: "127.0.0.1".into(),
                port,
                username: USER.into(),
            },
            opts(),
            rec.clone(),
        )
        .unwrap();

    // 1. unknown host key → accept and save
    let (id, req) = rec.wait_prompt(0);
    let PromptRequest::HostKeyUnknown {
        key_type,
        fingerprint,
        ..
    } = req
    else {
        panic!("expected host key prompt, got {req:?}");
    };
    assert_eq!(key_type, "ssh-ed25519");
    assert!(fingerprint.starts_with("SHA256:"));
    assert!(session.answer(
        id,
        PromptAnswer::HostKey {
            decision: HostKeyChoice::AcceptAndSave
        }
    ));

    // 2. password: wrong first, then right
    let (id, req) = rec.wait_prompt(1);
    assert!(matches!(req, PromptRequest::Password { retry: false, .. }));
    assert!(session.answer(
        id,
        PromptAnswer::Secret {
            value: "nope".into(),
            remember: false
        }
    ));
    let (id, req) = rec.wait_prompt(2);
    assert!(matches!(req, PromptRequest::Password { retry: true, .. }));
    assert!(session.answer(
        id,
        PromptAnswer::Secret {
            value: PASSWORD.into(),
            remember: false
        }
    ));

    rec.wait_state(|s| matches!(s, SessionState::Connected));
    let rows = wait_text(&session, "prompt$");
    assert!(rows[0].starts_with("prompt$"));
    let snap = session.snapshot();
    assert_eq!((snap.cols, snap.rows), (40, 6));
    // bold green prompt cell
    assert_ne!(snap.flags[0] & flag::BOLD, 0);
    assert!(
        rec.titles
            .lock()
            .unwrap()
            .contains(&Some("tester@box".into()))
    );

    // 3. typing echoes, resize is forwarded, OS detected
    session.write(b"echo hi".to_vec());
    wait_text(&session, "prompt$ echo hi");
    session.resize(50, 8);
    wait_text(&session, "resized 50x8");
    assert_eq!(session.snapshot().cols, 50);
    let t = Instant::now();
    while rec.os.lock().unwrap().is_empty() {
        assert!(t.elapsed() < Duration::from_secs(15), "no OS detected");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(rec.os.lock().unwrap()[0], "ubuntu");

    // 4. remote exit closes the session and lands in history + known hosts
    session.write(b"exit\r".to_vec());
    let st = rec.wait_state(|s| matches!(s, SessionState::Closed { .. }));
    assert!(matches!(st, SessionState::Closed { .. }));
    assert!(*rec.renders.lock().unwrap() > 0);

    let kh = app.known_hosts().unwrap();
    assert_eq!(kh.len(), 1);
    assert_eq!(kh[0].hostname, format!("[127.0.0.1]:{port}"));
    let hist = app.history(10).unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].protocol, "ssh");
    assert!(hist[0].error.is_none());
    app.forget_known_host(kh[0].id.clone()).unwrap();
    assert!(app.known_hosts().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_host_keyboard_interactive_and_rejected_key() {
    let port = start(true).await;
    let (app, _dir) = app();
    let vault = app.local_vault().unwrap().id;
    let mut d = app.new_host_draft(vault, None).unwrap();
    d.label = "kbd".into();
    d.address = "127.0.0.1".into();
    d.port = Some(port);
    d.username = USER.into();
    let host = app.save_host(d).unwrap();

    // Rejecting the key fails the session with a typed error.
    let rec = Arc::new(Recorder::default());
    let s = app
        .connect_host(host.id.clone(), opts(), rec.clone())
        .unwrap();
    let (id, _) = rec.wait_prompt(0);
    assert!(s.answer(
        id,
        PromptAnswer::HostKey {
            decision: HostKeyChoice::Reject
        }
    ));
    let st = rec.wait_state(|s| matches!(s, SessionState::Failed { .. }));
    let SessionState::Failed { kind, .. } = st else {
        unreachable!()
    };
    assert_eq!(kind, "host_key_rejected");
    assert!(app.known_hosts().unwrap().is_empty());
    let hist = app.history(10).unwrap();
    assert_eq!(hist.len(), 1);
    assert!(hist[0].error.is_some());

    // Accept once + keyboard-interactive.
    let rec = Arc::new(Recorder::default());
    let s = app
        .connect_host(host.id.clone(), opts(), rec.clone())
        .unwrap();
    let (id, _) = rec.wait_prompt(0);
    assert!(s.answer(
        id,
        PromptAnswer::HostKey {
            decision: HostKeyChoice::AcceptOnce
        }
    ));
    let (id, req) = rec.wait_prompt(1);
    let PromptRequest::KeyboardInteractive { questions, .. } = req else {
        panic!("expected kbd-interactive, got {req:?}");
    };
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].prompt, "Code: ");
    assert!(!questions[0].echo);
    assert!(s.answer(
        id,
        PromptAnswer::Answers {
            values: vec!["424242".into()]
        }
    ));
    rec.wait_state(|s| matches!(s, SessionState::Connected));
    wait_text(&s, "prompt$");
    assert!(
        app.known_hosts().unwrap().is_empty(),
        "accept once does not save"
    );

    // Local close.
    s.close();
    rec.wait_state(|s| matches!(s, SessionState::Closed { .. }));
    assert_eq!(app.history(10).unwrap().len(), 2);
    let h = app.host(host.id).unwrap();
    assert!(h.last_connected.is_some());
    assert_eq!(h.os_name.as_deref(), Some("ubuntu"));

    // Cancelling a prompt fails the session cleanly.
    let rec = Arc::new(Recorder::default());
    let s = app.connect_host(h.id, opts(), rec.clone()).unwrap();
    let (id, _) = rec.wait_prompt(0);
    assert!(s.answer(id, PromptAnswer::Cancel));
    let st = rec.wait_state(|s| matches!(s, SessionState::Failed { .. }));
    let SessionState::Failed { kind, .. } = st else {
        unreachable!()
    };
    assert_eq!(kind, "cancelled");
    assert!(!s.answer(id, PromptAnswer::Cancel), "stale prompt id");
}

//! End-to-end tests against an in-process `russh` server: host-key trust,
//! every auth method, shell/exec/resize, SFTP, and port forwarding.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use bytes::Bytes;
use russh::keys::ssh_key::{Algorithm, PrivateKey, PublicKey};
use russh::server::{Auth, ChannelOpenHandle, Msg, Server as _, Session};
use russh::{Channel, ChannelId, MethodSet};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use termoso_core::error::CoreError;
use termoso_core::forward::{Forward, ForwardSpec};
use termoso_core::hostkey::{
    FixedPrompt, HostKeyDecision, HostKeyVerdict, KnownHosts, StrictPrompt,
};
use termoso_core::sftp::{Sftp, TransferOptions};
use termoso_core::ssh::{AuthMethod, ConnectOptions, PasswordResponder, SshClient, SshTarget};
use termoso_core::store::Store;
use termoso_core::terminal::{TermEvent, TermSize, TerminalSession};
use termoso_crypto::keys::SymmetricKey;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use zeroize::Zeroizing;

const USER: &str = "tester";
const PASSWORD: &str = "s3cret";

/// Everything the test server records so tests can assert on it.
#[derive(Default)]
struct Observed {
    pty: Mutex<Option<(String, u32, u32)>>,
    resized: Mutex<Option<(u32, u32)>>,
    env: Mutex<Vec<(String, String)>>,
    kbd_rounds: AtomicU32,
    forward_requests: Mutex<Vec<(String, u32)>>,
    cancelled_forwards: AtomicBool,
}

#[derive(Clone)]
struct TestServer {
    authorized: Arc<PublicKey>,
    observed: Arc<Observed>,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    tcp_echo: Option<u16>,
}

struct TestHandler {
    srv: TestServer,
    channels: HashMap<ChannelId, Channel<Msg>>,
    shell_echo: HashMap<ChannelId, ()>,
}

impl russh::server::Server for TestServer {
    type Handler = TestHandler;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> TestHandler {
        TestHandler {
            srv: self.clone(),
            channels: HashMap::new(),
            shell_echo: HashMap::new(),
        }
    }
}

impl russh::server::Handler for TestHandler {
    type Error = russh::Error;

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Reject {
            proceed_with_methods: Some(MethodSet::from(
                &[
                    russh::MethodKind::Password,
                    russh::MethodKind::PublicKey,
                    russh::MethodKind::KeyboardInteractive,
                ][..],
            )),
            partial_success: false,
        })
    }

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(if user == USER && password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey_offered(
        &mut self,
        _user: &str,
        key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(if key.key_data() == self.srv.authorized.key_data() {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(
            if user == USER && key.key_data() == self.srv.authorized.key_data() {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn auth_keyboard_interactive<'a>(
        &'a mut self,
        _user: &str,
        _submethods: &str,
        response: Option<russh::server::Response<'a>>,
    ) -> Result<Auth, Self::Error> {
        let round = self.srv.observed.kbd_rounds.fetch_add(1, Ordering::SeqCst);
        match response {
            None => Ok(Auth::Partial {
                name: "Verification".into(),
                instructions: "Answer the prompts".into(),
                prompts: vec![
                    ("Password: ".into(), false),
                    ("Username (echoed): ".into(), true),
                ]
                .into(),
            }),
            Some(mut r) => {
                let pw = r.next().unwrap_or_default();
                let echoed = r.next().unwrap_or_default();
                let _ = round;
                Ok(if pw.as_ref() == PASSWORD.as_bytes() && echoed.is_empty() {
                    Auth::Accept
                } else {
                    Auth::reject()
                })
            }
        }
    }

    async fn authentication_banner(&mut self) -> Result<Option<String>, Self::Error> {
        Ok(Some("Welcome to the termoso test server".into()))
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _oa: &str,
        _op: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Some(echo) = self.srv.tcp_echo else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        if host != "echo.internal" || port != 7 {
            reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
            return Ok(());
        }
        reply.accept().await;
        tokio::spawn(async move {
            let Ok(mut sock) = tokio::net::TcpStream::connect(("127.0.0.1", echo)).await else {
                return;
            };
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut sock).await;
        });
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        cols: u32,
        rows: u32,
        _pw: u32,
        _ph: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        *self.srv.observed.pty.lock().await = Some((term.to_string(), cols, rows));
        session.channel_success(channel)?;
        Ok(())
    }

    async fn env_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        value: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.srv
            .observed
            .env
            .lock()
            .await
            .push((name.into(), value.into()));
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
        *self.srv.observed.resized.lock().await = Some((cols, rows));
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.shell_echo.insert(channel, ());
        session.channel_success(channel)?;
        session.data(channel, Bytes::from_static(b"prompt$ "))?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        cmd: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        let cmd = String::from_utf8_lossy(cmd).into_owned();
        match cmd.as_str() {
            "whoami" => {
                session.data(channel, Bytes::from_static(b"tester\n"))?;
                session.exit_status_request(channel, 0)?;
            }
            "fail" => {
                session.extended_data(channel, 1, Bytes::from_static(b"boom\n"))?;
                session.exit_status_request(channel, 7)?;
            }
            "cat" => {
                // echoes stdin until EOF; handled in `data`/`channel_eof`
                self.shell_echo.insert(channel, ());
                return Ok(());
            }
            _ => {
                session.extended_data(channel, 1, Bytes::from(format!("unknown: {cmd}\n")))?;
                session.exit_status_request(channel, 127)?;
            }
        }
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
        if self.shell_echo.contains_key(&channel) {
            if data == b"exit\n" {
                session.data(channel, Bytes::from_static(b"bye\n"))?;
                session.exit_status_request(channel, 3)?;
                session.eof(channel)?;
                session.close(channel)?;
            } else {
                session.data(channel, Bytes::copy_from_slice(data))?;
            }
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.shell_echo.remove(&channel).is_some() {
            session.exit_status_request(channel, 0)?;
            session.eof(channel)?;
            session.close(channel)?;
        }
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel_id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            session.channel_failure(channel_id)?;
            return Ok(());
        }
        let channel = self.channels.remove(&channel_id).expect("session channel");
        session.channel_success(channel_id)?;
        let files = self.srv.files.clone();
        tokio::spawn(async move {
            russh_sftp::server::run(channel.into_stream(), MemSftp::new(files)).await;
        });
        Ok(())
    }

    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if *port == 0 {
            *port = 40_000;
        }
        self.srv
            .observed
            .forward_requests
            .lock()
            .await
            .push((address.to_string(), *port));
        let handle = session.handle();
        let address = address.to_string();
        let port = *port;
        // Simulate one inbound connection to the remote listener.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let Ok(channel) = handle
                .channel_open_forwarded_tcpip(address, port, "10.0.0.9", 5555)
                .await
            else {
                return;
            };
            let mut stream = channel.into_stream();
            let _ = stream.write_all(b"ping-from-server").await;
            let mut buf = vec![0u8; 64];
            if let Ok(n) = stream.read(&mut buf).await {
                let _ = stream.write_all(&buf[..n]).await;
            }
            let _ = stream.shutdown().await;
        });
        Ok(true)
    }

    async fn cancel_tcpip_forward(
        &mut self,
        _a: &str,
        _p: u32,
        _s: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.srv
            .observed
            .cancelled_forwards
            .store(true, Ordering::SeqCst);
        Ok(true)
    }
}

// ---- minimal in-memory SFTP server -------------------------------------

struct OpenFile {
    path: String,
    flags: OpenFlags,
}

struct MemSftp {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    handles: HashMap<String, OpenFile>,
    dirs: HashMap<String, Option<Vec<File>>>,
    next: u32,
}

impl MemSftp {
    fn new(files: Arc<Mutex<HashMap<String, Vec<u8>>>>) -> Self {
        Self {
            files,
            handles: HashMap::new(),
            dirs: HashMap::new(),
            next: 1,
        }
    }
    fn handle(&mut self) -> String {
        self.next += 1;
        format!("h{}", self.next)
    }
    fn ok(id: u32) -> Status {
        Status {
            id,
            status_code: StatusCode::Ok,
            error_message: "Ok".into(),
            language_tag: "en-US".into(),
        }
    }
    fn norm(p: &str) -> String {
        let p = if p.is_empty() || p == "." {
            "/home/tester".to_string()
        } else {
            p.to_string()
        };
        let p = if p.starts_with('/') {
            p
        } else {
            format!("/home/tester/{p}")
        };
        let mut parts: Vec<&str> = Vec::new();
        for seg in p.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                s => parts.push(s),
            }
        }
        format!("/{}", parts.join("/"))
    }
    fn is_dir(files: &HashMap<String, Vec<u8>>, p: &str) -> bool {
        p == "/" || p == "/home" || p == "/home/tester" || files.contains_key(&format!("{p}/"))
    }
    fn attrs_for(files: &HashMap<String, Vec<u8>>, p: &str) -> Option<FileAttributes> {
        if Self::is_dir(files, p) {
            let mut a = FileAttributes::default();
            a.set_dir(true);
            a.permissions = Some(0o40755);
            return Some(a);
        }
        files.get(p).map(|d| FileAttributes {
            size: Some(d.len() as u64),
            permissions: Some(0o100644),
            mtime: Some(1_700_000_000),
            atime: Some(1_700_000_000),
            ..FileAttributes::default()
        })
    }
}

impl russh_sftp::server::Handler for MemSftp {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _ext: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let p = Self::norm(&path);
        Ok(Name {
            id,
            files: vec![File::dummy(&p)],
        })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let p = Self::norm(&path);
        let files = self.files.lock().await;
        match Self::attrs_for(&files, &p) {
            Some(attrs) => Ok(Attrs { id, attrs }),
            None => Err(StatusCode::NoSuchFile),
        }
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        self.stat(id, path).await
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let path = self
            .handles
            .get(&handle)
            .ok_or(StatusCode::BadMessage)?
            .path
            .clone();
        self.stat(id, path).await
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let p = Self::norm(&path);
        let files = self.files.lock().await;
        if !Self::is_dir(&files, &p) {
            return Err(StatusCode::NoSuchFile);
        }
        let prefix = if p == "/" {
            "/".to_string()
        } else {
            format!("{p}/")
        };
        let mut names: std::collections::BTreeSet<String> = Default::default();
        for k in files.keys() {
            if let Some(rest) = k.strip_prefix(&prefix) {
                if rest.is_empty() {
                    continue;
                }
                names.insert(rest.split('/').next().unwrap().to_string());
            }
        }
        if p == "/" {
            names.insert("home".into());
        }
        if p == "/home" {
            names.insert("tester".into());
        }
        let entries = names
            .into_iter()
            .map(|n| {
                let full = format!("{prefix}{n}");
                let attrs = Self::attrs_for(&files, &full).unwrap_or_default();
                File::new(n, attrs)
            })
            .collect();
        drop(files);
        let h = self.handle();
        self.dirs.insert(h.clone(), Some(entries));
        Ok(Handle { id, handle: h })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        match self.dirs.get_mut(&handle) {
            Some(slot) => match slot.take() {
                Some(files) => Ok(Name { id, files }),
                None => Err(StatusCode::Eof),
            },
            None => Err(StatusCode::BadMessage),
        }
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        self.dirs.remove(&handle);
        Ok(Self::ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        let p = Self::norm(&path);
        let mut files = self.files.lock().await;
        if Self::attrs_for(&files, &p).is_some() {
            return Err(StatusCode::Failure);
        }
        let parent = p
            .rsplit_once('/')
            .map(|(a, _)| if a.is_empty() { "/" } else { a })
            .unwrap_or("/");
        if !Self::is_dir(&files, parent) {
            return Err(StatusCode::NoSuchFile);
        }
        files.insert(format!("{p}/"), Vec::new());
        Ok(Self::ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        let p = Self::norm(&path);
        let mut files = self.files.lock().await;
        let prefix = format!("{p}/");
        if files.keys().any(|k| k.starts_with(&prefix) && k != &prefix) {
            return Err(StatusCode::Failure);
        }
        files
            .remove(&prefix)
            .map(|_| Self::ok(id))
            .ok_or(StatusCode::NoSuchFile)
    }

    async fn remove(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        let p = Self::norm(&path);
        self.files
            .lock()
            .await
            .remove(&p)
            .map(|_| Self::ok(id))
            .ok_or(StatusCode::NoSuchFile)
    }

    async fn rename(&mut self, id: u32, from: String, to: String) -> Result<Status, Self::Error> {
        let (f, t) = (Self::norm(&from), Self::norm(&to));
        let mut files = self.files.lock().await;
        let Some(data) = files.remove(&f) else {
            return Err(StatusCode::NoSuchFile);
        };
        files.insert(t, data);
        Ok(Self::ok(id))
    }

    async fn setstat(
        &mut self,
        id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(Self::ok(id))
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let p = Self::norm(&filename);
        let mut files = self.files.lock().await;
        let exists = files.contains_key(&p);
        if flags.contains(OpenFlags::CREATE) {
            if flags.contains(OpenFlags::EXCLUDE) && exists {
                return Err(StatusCode::Failure);
            }
            if !exists || flags.contains(OpenFlags::TRUNCATE) {
                files.insert(p.clone(), Vec::new());
            }
        } else if !exists {
            return Err(StatusCode::NoSuchFile);
        }
        drop(files);
        let h = self.handle();
        self.handles.insert(h.clone(), OpenFile { path: p, flags });
        Ok(Handle { id, handle: h })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let path = self
            .handles
            .get(&handle)
            .ok_or(StatusCode::BadMessage)?
            .path
            .clone();
        let files = self.files.lock().await;
        let data = files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let start = (offset as usize).min(data.len());
        if start == data.len() {
            return Err(StatusCode::Eof);
        }
        let end = (start + len as usize).min(data.len());
        Ok(Data {
            id,
            data: data[start..end].to_vec(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let f = self.handles.get(&handle).ok_or(StatusCode::BadMessage)?;
        if !f.flags.contains(OpenFlags::WRITE) {
            return Err(StatusCode::PermissionDenied);
        }
        let path = f.path.clone();
        let mut files = self.files.lock().await;
        let buf = files.get_mut(&path).ok_or(StatusCode::NoSuchFile)?;
        let end = offset as usize + data.len();
        if buf.len() < end {
            buf.resize(end, 0);
        }
        buf[offset as usize..end].copy_from_slice(&data);
        Ok(Self::ok(id))
    }
}

// ---- harness -----------------------------------------------------------

struct Harness {
    port: u16,
    host_key: PublicKey,
    client_key: PrivateKey,
    observed: Arc<Observed>,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    store: Arc<Store>,
    kh: KnownHosts,
    _echo: Option<tokio::task::JoinHandle<()>>,
}

async fn echo_server() -> (u16, tokio::task::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let h = tokio::spawn(async move {
        while let Ok((mut s, _)) = l.accept().await {
            tokio::spawn(async move {
                let (mut r, mut w) = s.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
            });
        }
    });
    (port, h)
}

async fn start() -> Harness {
    let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let client_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let (echo_port, echo) = echo_server().await;
    let observed = Arc::new(Observed::default());
    let files = Arc::new(Mutex::new(HashMap::new()));
    let mut server = TestServer {
        authorized: Arc::new(client_key.public_key().clone()),
        observed: observed.clone(),
        files: files.clone(),
        tcp_echo: Some(echo_port),
    };
    let config = Arc::new(russh::server::Config {
        auth_rejection_time: Duration::from_millis(0),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        keys: vec![host_key.clone()],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = server.run_on_socket(config, &listener).await;
    });

    let store = Arc::new(Store::open_in_memory(SymmetricKey::generate()).unwrap());
    let vault = store.local_vault().unwrap().id;
    let kh = KnownHosts::new(store.clone(), vault);
    Harness {
        port,
        host_key: host_key.public_key().clone(),
        client_key,
        observed,
        files,
        store,
        kh,
        _echo: Some(echo),
    }
}

impl Harness {
    fn opts(&self, prompt: Arc<dyn termoso_core::hostkey::HostKeyPrompt>) -> ConnectOptions {
        let mut o = ConnectOptions::new(
            SshTarget {
                host: "127.0.0.1".into(),
                port: self.port,
                username: USER.into(),
            },
            self.kh.clone(),
            prompt,
        );
        o.timeout = Duration::from_secs(10);
        o
    }

    fn trusted(&self) -> ConnectOptions {
        self.kh
            .trust("127.0.0.1", self.port, &self.host_key)
            .unwrap();
        self.opts(Arc::new(StrictPrompt))
    }

    async fn connect_password(&self) -> SshClient {
        let mut o = self.trusted();
        o.auth
            .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
        SshClient::connect(o).await.unwrap()
    }
}

async fn collect_until_exit(
    events: &mut termoso_core::terminal::TermEvents,
) -> (Vec<u8>, Option<u32>) {
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .unwrap()
        {
            Some(TermEvent::Output(b)) => out.extend_from_slice(&b),
            Some(TermEvent::Exit { code, .. }) => return (out, code),
            Some(TermEvent::Closed) | None => return (out, None),
            Some(_) => {}
        }
    }
}

// ---- tests -------------------------------------------------------------

#[tokio::test]
async fn unknown_host_key_is_rejected_by_strict_prompt() {
    let h = start().await;
    let mut o = h.opts(Arc::new(StrictPrompt));
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    let err = SshClient::connect(o).await.err().unwrap();
    assert!(
        matches!(err, CoreError::HostKeyRejected { .. } | CoreError::Ssh(_)),
        "{err:?}"
    );
    assert!(matches!(
        h.kh.check("127.0.0.1", h.port, &h.host_key).unwrap(),
        HostKeyVerdict::Unknown { .. }
    ));
}

#[tokio::test]
async fn accept_and_save_pins_key_and_changed_key_is_refused() {
    let h = start().await;
    let mut o = h.opts(Arc::new(FixedPrompt(HostKeyDecision::AcceptAndSave)));
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    let c = SshClient::connect(o).await.unwrap();
    assert_eq!(c.banner(), Some("Welcome to the termoso test server"));
    assert_eq!(
        h.kh.check("127.0.0.1", h.port, &h.host_key).unwrap(),
        HostKeyVerdict::Known
    );
    c.disconnect().await.unwrap();

    // Pretend the server key rotated: pin a different key, then connect.
    let other = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    h.kh.trust("127.0.0.1", h.port, other.public_key()).unwrap();
    let mut o = h.opts(Arc::new(FixedPrompt(HostKeyDecision::AcceptAndSave)));
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    assert!(
        SshClient::connect(o).await.is_err(),
        "changed key must not be auto-accepted"
    );
    assert!(matches!(
        h.kh.check("127.0.0.1", h.port, &h.host_key).unwrap(),
        HostKeyVerdict::Changed { .. }
    ));
}

#[tokio::test]
async fn accept_once_does_not_persist() {
    let h = start().await;
    let mut o = h.opts(Arc::new(FixedPrompt(HostKeyDecision::AcceptOnce)));
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    SshClient::connect(o).await.unwrap();
    assert!(matches!(
        h.kh.check("127.0.0.1", h.port, &h.host_key).unwrap(),
        HostKeyVerdict::Unknown { .. }
    ));
}

#[tokio::test]
async fn wrong_password_then_key_succeeds_and_reports_remaining() {
    let h = start().await;
    let mut o = h.trusted();
    o.auth
        .push(AuthMethod::Password(Zeroizing::new("nope".into())));
    let err = SshClient::connect(o).await.err().unwrap();
    match err {
        CoreError::AuthFailed { remaining } => {
            assert!(remaining.iter().any(|m| m == "publickey"), "{remaining:?}")
        }
        e => panic!("{e:?}"),
    }

    let mut o = h.trusted();
    o.auth
        .push(AuthMethod::Password(Zeroizing::new("nope".into())));
    o.auth.push(AuthMethod::Key {
        private_key: Zeroizing::new(
            h.client_key
                .to_openssh(Default::default())
                .unwrap()
                .to_string(),
        ),
        passphrase: None,
        certificate: None,
    });
    SshClient::connect(o).await.unwrap();
}

#[tokio::test]
async fn encrypted_key_needs_passphrase() {
    let h = start().await;
    let enc = h.client_key.encrypt(&mut rand::rng(), b"pp").unwrap();
    let text = enc.to_openssh(Default::default()).unwrap().to_string();
    let mut o = h.trusted();
    o.auth.push(AuthMethod::Key {
        private_key: Zeroizing::new(text.clone()),
        passphrase: None,
        certificate: None,
    });
    assert!(matches!(
        SshClient::connect(o).await,
        Err(CoreError::Key(_))
    ));

    let mut o = h.trusted();
    o.auth.push(AuthMethod::Key {
        private_key: Zeroizing::new(text),
        passphrase: Some(Zeroizing::new("pp".into())),
        certificate: None,
    });
    SshClient::connect(o).await.unwrap();
}

#[tokio::test]
async fn keyboard_interactive_uses_prompt_responder() {
    let h = start().await;
    let mut o = h.trusted();
    o.auth.push(AuthMethod::KeyboardInteractive);
    o.interactive = Some(Arc::new(PasswordResponder(Zeroizing::new(PASSWORD.into()))));
    SshClient::connect(o).await.unwrap();
    assert!(h.observed.kbd_rounds.load(Ordering::SeqCst) >= 2);

    let mut o = h.trusted();
    o.auth.push(AuthMethod::KeyboardInteractive);
    assert!(SshClient::connect(o).await.is_err(), "no responder = fail");
}

#[tokio::test]
async fn shell_streams_echo_resizes_and_exits() {
    let h = start().await;
    let c = h.connect_password().await;
    let (term, mut events) = c
        .shell(
            "xterm-256color",
            TermSize {
                cols: 120,
                rows: 40,
            },
        )
        .await
        .unwrap();
    assert_eq!(term.kind(), "ssh");
    assert_eq!(
        *h.observed.pty.lock().await,
        Some(("xterm-256color".into(), 120, 40))
    );

    term.write(b"hello\n").await.unwrap();
    term.resize(TermSize { cols: 90, rows: 30 }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(*h.observed.resized.lock().await, Some((90, 30)));
    term.write(b"exit\n").await.unwrap();

    let (out, code) = collect_until_exit(&mut events).await;
    let s = String::from_utf8_lossy(&out);
    assert!(s.starts_with("prompt$ "), "{s:?}");
    assert!(s.contains("hello\n") && s.ends_with("bye\n"), "{s:?}");
    assert_eq!(code, Some(3));
    assert!(matches!(term.write(b"x").await, Err(CoreError::Closed)));
}

#[tokio::test]
async fn exec_returns_stdout_stderr_and_status() {
    let h = start().await;
    let c = h.connect_password().await;
    let ok = c.exec("whoami", None).await.unwrap();
    assert_eq!(ok.stdout, b"tester\n");
    assert_eq!(ok.exit_code, Some(0));
    let bad = c.exec("fail", None).await.unwrap();
    assert_eq!(bad.stderr, b"boom\n");
    assert_eq!(bad.exit_code, Some(7));
    let cat = c
        .exec("cat", Some(Bytes::from_static(b"piped")))
        .await
        .unwrap();
    assert_eq!(cat.stdout, b"piped");
    assert_eq!(cat.exit_code, Some(0));
}

#[tokio::test]
async fn env_is_sent_before_shell() {
    let h = start().await;
    let mut o = h.trusted();
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    o.env.push(("LANG".into(), "C.UTF-8".into()));
    let c = SshClient::connect(o).await.unwrap();
    let _ = c.shell("xterm", TermSize::default()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        h.observed.env.lock().await.as_slice(),
        &[("LANG".to_string(), "C.UTF-8".to_string())]
    );
}

#[tokio::test]
async fn disconnect_closes_transport() {
    let h = start().await;
    let c = h.connect_password().await;
    assert!(!c.is_closed());
    c.disconnect().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), c.closed())
        .await
        .unwrap();
    assert!(c.is_closed());
}

#[tokio::test]
async fn sftp_end_to_end() {
    let h = start().await;
    let c = h.connect_password().await;
    let sftp = Sftp::open(&c).await.unwrap();
    assert_eq!(sftp.home(), "/home/tester");

    sftp.mkdir_all("/home/tester/a/b").await.unwrap();
    sftp.write("/home/tester/a/b/f.txt", b"hello sftp")
        .await
        .unwrap();
    assert_eq!(
        sftp.read("/home/tester/a/b/f.txt").await.unwrap(),
        b"hello sftp"
    );
    let st = sftp.stat("a/b/f.txt").await.unwrap();
    assert_eq!(st.size, Some(10));
    assert_eq!(st.kind, termoso_core::sftp::EntryKind::File);
    assert!(sftp.exists("a/b").await.unwrap());
    assert!(!sftp.exists("a/zzz").await.unwrap());
    assert!(matches!(
        sftp.stat("nope").await,
        Err(CoreError::NotFound(_))
    ));

    let list = sftp.list("/home/tester/a/b").await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "f.txt");
    assert_eq!(list[0].path, "/home/tester/a/b/f.txt");

    sftp.rename("/home/tester/a/b/f.txt", "/home/tester/a/g.txt")
        .await
        .unwrap();
    let names: Vec<_> = sftp
        .list("a")
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["b", "g.txt"]);
    sftp.chmod("a/g.txt", 0o600).await.unwrap();

    // upload / download with progress + resume
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("big.bin");
    let payload: Vec<u8> = (0..(700 * 1024)).map(|i| (i % 251) as u8).collect();
    std::fs::write(&local, &payload).unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    let opts = TransferOptions {
        progress: Some(Arc::new(move |p| s2.lock().unwrap().push(p))),
        ..TransferOptions::default()
    };
    let n = sftp.upload(&local, "a/big.bin", &opts).await.unwrap();
    assert_eq!(n, payload.len() as u64);
    {
        let prog = seen.lock().unwrap();
        assert!(prog.len() >= 3, "{}", prog.len());
        assert_eq!(prog.last().unwrap().done, payload.len() as u64);
        assert_eq!(prog.last().unwrap().total, Some(payload.len() as u64));
    }
    assert_eq!(
        h.files.lock().await.get("/home/tester/a/big.bin").unwrap(),
        &payload
    );

    // Partial local copy → resume downloads only the tail.
    let out = dir.path().join("out.bin");
    std::fs::write(&out, &payload[..300_000]).unwrap();
    let opts = TransferOptions {
        resume: true,
        ..TransferOptions::default()
    };
    let n = sftp.download("a/big.bin", &out, &opts).await.unwrap();
    assert_eq!(n, (payload.len() - 300_000) as u64);
    assert_eq!(std::fs::read(&out).unwrap(), payload);

    // Cancel before start.
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    let opts = TransferOptions {
        cancel,
        ..TransferOptions::default()
    };
    assert!(matches!(
        sftp.download("a/big.bin", &dir.path().join("x"), &opts)
            .await,
        Err(CoreError::Cancelled)
    ));

    sftp.remove_dir_all("/home/tester/a", &Default::default())
        .await
        .unwrap();
    assert!(!sftp.exists("a").await.unwrap());
    sftp.close().await.unwrap();
}

#[tokio::test]
async fn local_forward_pipes_through_server() {
    let h = start().await;
    let c = Arc::new(h.connect_password().await);
    let fwd = Forward::start(
        c.clone(),
        ForwardSpec::Local {
            bind: "127.0.0.1".into(),
            port: 0,
            remote_host: "echo.internal".into(),
            remote_port: 7,
        },
    )
    .await
    .unwrap();
    let addr = fwd.local_addr().unwrap();
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(b"round trip").await.unwrap();
    let mut buf = [0u8; 10];
    s.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"round trip");
    drop(s);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(fwd.stats().connections.load(Ordering::Relaxed), 1);
    fwd.stop().await;
    assert!(
        tokio::net::TcpStream::connect(addr).await.is_err() || {
            // listener may still be draining; a second connect must fail soon
            tokio::time::sleep(Duration::from_millis(100)).await;
            tokio::net::TcpStream::connect(addr).await.is_err()
        }
    );
}

#[tokio::test]
async fn dynamic_socks5_forward() {
    let h = start().await;
    let c = Arc::new(h.connect_password().await);
    let fwd = Forward::start(
        c,
        ForwardSpec::Dynamic {
            bind: "127.0.0.1".into(),
            port: 0,
        },
    )
    .await
    .unwrap();
    let mut s = tokio::net::TcpStream::connect(fwd.local_addr().unwrap())
        .await
        .unwrap();
    s.write_all(&[5, 1, 0]).await.unwrap();
    let mut r = [0u8; 2];
    s.read_exact(&mut r).await.unwrap();
    assert_eq!(r, [5, 0]);
    let mut req = vec![5, 1, 0, 3, 13];
    req.extend_from_slice(b"echo.internal");
    req.extend_from_slice(&7u16.to_be_bytes());
    s.write_all(&req).await.unwrap();
    let mut rep = [0u8; 10];
    s.read_exact(&mut rep).await.unwrap();
    assert_eq!(rep[1], 0);
    s.write_all(b"socks!").await.unwrap();
    let mut buf = [0u8; 6];
    s.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"socks!");

    // refused destination → reply 5
    let mut s = tokio::net::TcpStream::connect(fwd.local_addr().unwrap())
        .await
        .unwrap();
    s.write_all(&[5, 1, 0]).await.unwrap();
    s.read_exact(&mut r).await.unwrap();
    let mut req = vec![5, 1, 0, 1, 10, 0, 0, 1];
    req.extend_from_slice(&80u16.to_be_bytes());
    s.write_all(&req).await.unwrap();
    s.read_exact(&mut rep).await.unwrap();
    assert_eq!(rep[1], 5);
}

#[tokio::test]
async fn remote_forward_delivers_server_connections_locally() {
    let h = start().await;
    let (echo_port, _echo) = echo_server().await;
    let c = Arc::new(h.connect_password().await);
    let fwd = Forward::start(
        c.clone(),
        ForwardSpec::Remote {
            bind: "localhost".into(),
            port: 0,
            local_host: "127.0.0.1".into(),
            local_port: echo_port,
        },
    )
    .await
    .unwrap();
    assert_eq!(fwd.remote_port(), Some(40_000));
    assert_eq!(
        h.observed.forward_requests.lock().await.as_slice(),
        &[("localhost".to_string(), 40_000)]
    );

    // The server "connects" once (see tcpip_forward) → our echo server → back.
    tokio::time::timeout(Duration::from_secs(5), async {
        while fwd.stats().connections.load(Ordering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while fwd.stats().bytes_in.load(Ordering::Relaxed) < 16 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    fwd.stop().await;
    assert!(h.observed.cancelled_forwards.load(Ordering::SeqCst));
}

#[tokio::test]
async fn jump_host_chain() {
    // Connect to the same server "through itself" via direct-tcpip is not
    // possible (the test server only tunnels to echo.internal), so verify the
    // failure path is typed and the jump session survives.
    let h = start().await;
    let jump = h.connect_password().await;
    let mut o = h.trusted();
    o.target.host = "unreachable.internal".into();
    o.auth
        .push(AuthMethod::Password(Zeroizing::new(PASSWORD.into())));
    let err = SshClient::connect_via(&jump, o).await.err().unwrap();
    assert!(matches!(err, CoreError::Ssh(_)), "{err:?}");
    assert!(!jump.is_closed());
    let _ = h.store.local_vault().unwrap();
}

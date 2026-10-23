//! ZMODEM (`rz` / `sz`) inside a terminal tab. The remote starts the protocol
//! (`sz file` → we receive, `rz` → we send); the output pump spots the opening
//! header, hides the protocol bytes from the screen and asks the UI where the
//! files go (or which to send). The transfer then runs on the session's own
//! byte stream: output chunks are fed to the `zmodem2` state machine, its
//! wire bytes are written back as terminal input. Progress and completion
//! arrive as `session` events.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use termoso_core::terminal::SharedTerminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use zmodem2::{Action, Event, FileInfo};

use crate::error::{DesktopError, Result};
use crate::sessions::{SESSION_EVENT, SessionEvent};
use crate::state::AppState;

/// `sz` opens with ZRQINIT (`**\x18B00…`), `rz` with ZRINIT (`**\x18B01…`).
const ZRQINIT: &[u8] = b"\x18B00";
const ZRINIT: &[u8] = b"\x18B01";
/// Protocol bytes kept while the UI decides; the remote repeats its header anyway.
const OFFER_BUFFER_CAP: usize = 64 * 1024;
/// Replies the remote has to answer within; then the handshake is re-sent.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The remote runs `sz`: files come to this machine.
    Receive,
    /// The remote runs `rz`: files go from this machine.
    Send,
}

enum State {
    Idle {
        /// Last bytes of the previous chunk, so a header split across chunks is still seen.
        tail: Vec<u8>,
    },
    /// Header seen, UI asked; protocol bytes are held back until it answers.
    Offered { kind: Kind, held: Vec<u8> },
    Running {
        wire: mpsc::UnboundedSender<Bytes>,
        cancel: CancellationToken,
    },
}

/// Per-session ZMODEM state, shared by the output pump and the commands.
pub struct Slot(Mutex<State>);

impl Default for Slot {
    fn default() -> Self {
        Self(Mutex::new(State::Idle { tail: Vec::new() }))
    }
}

impl Slot {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Stop whatever runs; used when the session goes away.
    pub fn shutdown(&self) {
        let mut st = self.lock();
        if let State::Running { cancel, .. } = &*st {
            cancel.cancel();
        }
        *st = State::Idle { tail: Vec::new() };
    }
}

/// Where a ZMODEM header starts in `data`, with what it announces.
fn find_header(data: &[u8]) -> Option<(usize, Kind)> {
    let hit = |needle: &[u8]| data.windows(needle.len()).position(|w| w == needle);
    let rq = hit(ZRQINIT).map(|i| (i, Kind::Receive));
    let ri = hit(ZRINIT).map(|i| (i, Kind::Send));
    let (i, kind) = match (rq, ri) {
        (Some(a), Some(b)) => {
            if a.0 <= b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    // `**` precedes the header; show everything before it.
    let start = if i >= 2 && &data[i - 2..i] == b"**" {
        i - 2
    } else {
        i
    };
    Some((start, kind))
}

/// Called by the output pump for every chunk. Returns what the screen should
/// see; protocol bytes are swallowed and, on the first header, the UI is told.
pub fn filter<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    slot: &Slot,
    bytes: Bytes,
) -> Option<Bytes> {
    let mut st = slot.lock();
    match &mut *st {
        State::Running { wire, .. } => {
            if wire.send(bytes).is_err() {
                *st = State::Idle { tail: Vec::new() };
            }
            None
        }
        State::Offered { held, .. } => {
            if held.len() < OFFER_BUFFER_CAP {
                held.extend_from_slice(&bytes);
            }
            None
        }
        State::Idle { tail } => {
            let mut probe = Vec::with_capacity(tail.len() + bytes.len());
            probe.extend_from_slice(tail);
            probe.extend_from_slice(&bytes);
            match find_header(&probe) {
                Some((start, kind)) => {
                    let shown = start.saturating_sub(tail.len());
                    let visible = bytes.slice(..shown);
                    *st = State::Offered {
                        kind,
                        held: probe[start..].to_vec(),
                    };
                    let _ = app.emit(SESSION_EVENT, SessionEvent::Zmodem { id, kind });
                    Some(visible)
                }
                None => {
                    let keep = probe.len().min(ZRQINIT.len() + 1);
                    *tail = probe[probe.len() - keep..].to_vec();
                    Some(bytes)
                }
            }
        }
    }
}

/// Files the remote `sz` offers go into `dir`.
pub async fn receive<R: Runtime>(app: AppHandle<R>, id: Uuid, dir: String) -> Result<()> {
    let dir = PathBuf::from(dir);
    if !dir.is_dir() {
        return Err(DesktopError::invalid(format!(
            "{} is not a folder",
            dir.display()
        )));
    }
    let (term, slot) = session_parts(&app, id)?;
    let (rx, cancel) = begin(&slot, Kind::Receive)?;
    tauri::async_runtime::spawn(async move {
        let res = run_receiver(&app, id, term, rx, cancel, dir).await;
        finish(&app, id, &slot, res);
    });
    Ok(())
}

/// Files for the remote `rz`.
pub async fn send<R: Runtime>(app: AppHandle<R>, id: Uuid, paths: Vec<String>) -> Result<()> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err(DesktopError::invalid("no files to send"));
    }
    for p in &paths {
        if !p.is_file() {
            return Err(DesktopError::invalid(format!(
                "{} is not a file",
                p.display()
            )));
        }
    }
    let (term, slot) = session_parts(&app, id)?;
    let (rx, cancel) = begin(&slot, Kind::Send)?;
    tauri::async_runtime::spawn(async move {
        let res = run_sender(&app, id, term, rx, cancel, paths).await;
        finish(&app, id, &slot, res);
    });
    Ok(())
}

/// Decline the offer (the remote times out) or abort a running transfer.
pub async fn cancel<R: Runtime>(app: AppHandle<R>, id: Uuid) -> Result<()> {
    let (term, slot) = session_parts(&app, id)?;
    let prev = std::mem::replace(&mut *slot.lock(), State::Idle { tail: Vec::new() });
    match prev {
        State::Running { cancel, .. } => cancel.cancel(),
        State::Offered { .. } => {
            // Tell the remote program to stop instead of letting it retry for a minute.
            let _ = term.write(&abort_sequence()).await;
            let _ = app.emit(
                SESSION_EVENT,
                SessionEvent::ZmodemDone {
                    id,
                    ok: false,
                    files: 0,
                    error: Some("cancelled".into()),
                },
            );
        }
        State::Idle { .. } => {}
    }
    Ok(())
}

fn session_parts<R: Runtime>(app: &AppHandle<R>, id: Uuid) -> Result<(SharedTerminal, Arc<Slot>)> {
    let state = app.state::<AppState>();
    Ok((state.sessions.terminal(id)?, state.sessions.zmodem(id)?))
}

fn begin(slot: &Slot, kind: Kind) -> Result<(mpsc::UnboundedReceiver<Bytes>, CancellationToken)> {
    let mut st = slot.lock();
    let held = match &*st {
        State::Offered { kind: k, held } if *k == kind => held.clone(),
        State::Offered { .. } => {
            return Err(DesktopError::invalid(
                "the remote asked for the other direction",
            ));
        }
        State::Running { .. } => {
            return Err(DesktopError::invalid("a transfer is already running"));
        }
        State::Idle { .. } => return Err(DesktopError::invalid("no ZMODEM request pending")),
    };
    let (tx, rx) = mpsc::unbounded_channel();
    if !held.is_empty() {
        let _ = tx.send(Bytes::from(held));
    }
    let cancel = CancellationToken::new();
    *st = State::Running {
        wire: tx,
        cancel: cancel.clone(),
    };
    Ok((rx, cancel))
}

fn finish<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    slot: &Slot,
    res: std::result::Result<u32, String>,
) {
    *slot.lock() = State::Idle { tail: Vec::new() };
    let (ok, files, error) = match res {
        Ok(n) => (true, n, None),
        Err(e) => (false, 0, Some(e)),
    };
    let _ = app.emit(
        SESSION_EVENT,
        SessionEvent::ZmodemDone {
            id,
            ok,
            files,
            error,
        },
    );
}

/// CAN×8 + BS×8 (ZMODEM "attention / abort").
fn abort_sequence() -> Vec<u8> {
    let mut v = vec![0x18; 8];
    v.extend(std::iter::repeat_n(0x08, 8));
    v
}

struct Progress<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    id: Uuid,
    kind: Kind,
    last: Instant,
}

impl<R: Runtime> Progress<'_, R> {
    fn report(&mut self, name: &str, done: u64, total: Option<u64>, files: u32, force: bool) {
        if !force && self.last.elapsed() < PROGRESS_EVERY {
            return;
        }
        self.last = Instant::now();
        let _ = self.app.emit(
            SESSION_EVENT,
            SessionEvent::ZmodemProgress {
                id: self.id,
                kind: self.kind,
                name: name.to_string(),
                done,
                total,
                files,
            },
        );
    }
}

/// File being received right now.
struct Incoming {
    file: std::fs::File,
    path: PathBuf,
    name: String,
    total: Option<u64>,
    modified: Option<u32>,
}

/// The receiving side of a `sz`.
async fn run_receiver<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    term: SharedTerminal,
    mut rx: mpsc::UnboundedReceiver<Bytes>,
    cancel: CancellationToken,
    dir: PathBuf,
) -> std::result::Result<u32, String> {
    let mut engine = zmodem2::Receiver::new().map_err(|e| format!("{e:?}"))?;
    let mut progress = Progress {
        app,
        id,
        kind: Kind::Receive,
        last: Instant::now() - PROGRESS_EVERY,
    };
    let mut inbuf: Vec<u8> = Vec::new();
    let mut files = 0u32;
    let mut current: Option<Incoming> = None;
    let mut done = 0u64;
    loop {
        let action = engine.poll();
        match action {
            Action::WriteWire(b) => {
                let v = b.to_vec();
                term.write(&v).await.map_err(|e| e.to_string())?;
                engine.wire_written(v.len());
            }
            Action::WriteFile(b) => {
                let v = b.to_vec();
                let Some(cur) = current.as_mut() else {
                    engine.abort().ok();
                    return Err("data before a file header".into());
                };
                cur.file.write_all(&v).map_err(|e| e.to_string())?;
                done += v.len() as u64;
                engine.file_written(v.len()).map_err(|e| format!("{e:?}"))?;
                progress.report(&cur.name, done, cur.total, files, false);
            }
            Action::ReadFile { .. } => {
                engine.abort().ok();
                return Err("receiver asked to read a file".into());
            }
            Action::Event(ev) => match ev {
                Event::FileStarted(info) => {
                    let name = receive_name(info.name);
                    let path = unique_path(&dir, &name);
                    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                    let total = info.size.map(|s| u64::from(u32::from(s)));
                    done = 0;
                    progress.report(&name, 0, total, files, true);
                    current = Some(Incoming {
                        file,
                        path,
                        name,
                        total,
                        modified: info.modified,
                    });
                }
                Event::FileCompleted => {
                    if let Some(mut cur) = current.take() {
                        let _ = cur.file.flush();
                        if let Some(m) = cur.modified {
                            let t = std::time::UNIX_EPOCH + Duration::from_secs(u64::from(m));
                            let _ = cur.file.set_modified(t);
                        }
                        files += 1;
                        progress.report(&cur.name, done, cur.total.or(Some(done)), files, true);
                        tracing::debug!(path = %cur.path.display(), "zmodem: received");
                    }
                }
                Event::SessionCompleted => return Ok(files),
                Event::Aborted => return Err("aborted by the remote side".into()),
                _ => {}
            },
            Action::Idle => {
                if !inbuf.is_empty() {
                    let n = engine.submit_wire(&inbuf).map_err(|e| format!("{e:?}"))?;
                    inbuf.drain(..n);
                    if n > 0 {
                        continue;
                    }
                }
                tokio::select! {
                    _ = cancel.cancelled() => {
                        engine.abort().ok();
                        drain_wire(pending_wire_recv(&mut engine), &term).await;
                        return Err("cancelled".into());
                    }
                    chunk = rx.recv() => match chunk {
                        Some(b) => inbuf.extend_from_slice(&b),
                        None => return Err("session closed".into()),
                    },
                    _ = tokio::time::sleep(RESPONSE_TIMEOUT) => {
                        engine.timeout().map_err(|e| format!("{e:?}"))?;
                    }
                }
            }
            _ => {}
        }
    }
}

/// The sending side of an `rz`.
async fn run_sender<R: Runtime>(
    app: &AppHandle<R>,
    id: Uuid,
    term: SharedTerminal,
    mut rx: mpsc::UnboundedReceiver<Bytes>,
    cancel: CancellationToken,
    paths: Vec<PathBuf>,
) -> std::result::Result<u32, String> {
    let mut engine = zmodem2::Sender::new().map_err(|e| format!("{e:?}"))?;
    let mut progress = Progress {
        app,
        id,
        kind: Kind::Send,
        last: Instant::now() - PROGRESS_EVERY,
    };
    let mut queue = paths.into_iter();
    let mut inbuf: Vec<u8> = Vec::new();
    let mut files = 0u32;
    let mut current: Option<(std::fs::File, String, u64)> = None;
    let mut done = 0u64;
    // Next thing to tell the engine once its outgoing bytes are drained.
    let mut next: Option<Option<PathBuf>> = Some(queue.next());
    let mut finished = false;
    loop {
        if let Some(step) = next.take() {
            let res = match &step {
                Some(path) => open_for_send(path).and_then(|(file, name, size)| {
                    let info = FileInfo::new(
                        name.as_bytes(),
                        Some(
                            u32::try_from(size)
                                .map_err(|_| "file larger than 4 GiB".to_string())?
                                .into(),
                        ),
                    );
                    let info = match file
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    {
                        Some(d) => {
                            info.with_modified(u32::try_from(d.as_secs()).unwrap_or(u32::MAX))
                        }
                        None => info,
                    };
                    match engine.start_file(info) {
                        Ok(()) => {
                            done = 0;
                            progress.report(&name, 0, Some(size), files, true);
                            current = Some((file, name, size));
                            Ok(true)
                        }
                        Err(zmodem2::Error::Backpressure) => Ok(false),
                        Err(e) => Err(format!("{e:?}")),
                    }
                }),
                None => match engine.finish() {
                    Ok(()) => {
                        finished = true;
                        Ok(true)
                    }
                    Err(zmodem2::Error::Backpressure) => Ok(false),
                    Err(e) => Err(format!("{e:?}")),
                },
            };
            match res {
                Ok(true) => {}
                Ok(false) => next = Some(step),
                Err(e) => {
                    engine.abort();
                    drain_wire(pending_wire_send(&mut engine), &term).await;
                    return Err(e);
                }
            }
        }
        let action = engine.poll();
        match action {
            Action::WriteWire(b) => {
                let v = b.to_vec();
                term.write(&v).await.map_err(|e| e.to_string())?;
                engine.wire_written(v.len());
            }
            Action::ReadFile { offset, max_len } => {
                let Some((file, name, size)) = current.as_mut() else {
                    engine.abort();
                    return Err("read request without a file".into());
                };
                let off = u64::from(u32::from(offset));
                file.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
                let mut buf = vec![0u8; max_len];
                let n = file.read(&mut buf).map_err(|e| e.to_string())?;
                buf.truncate(n);
                engine.submit_file(&buf).map_err(|e| format!("{e:?}"))?;
                done = off + n as u64;
                progress.report(name, done, Some(*size), files, false);
            }
            Action::WriteFile(_) => {
                engine.abort();
                return Err("sender asked to write a file".into());
            }
            Action::Event(ev) => match ev {
                Event::FileCompleted => {
                    if let Some((_, name, size)) = current.take() {
                        files += 1;
                        progress.report(&name, size, Some(size), files, true);
                    }
                    next = Some(queue.next());
                }
                Event::SessionCompleted => return Ok(files),
                Event::Aborted => return Err("aborted by the remote side".into()),
                _ => {}
            },
            Action::Idle => {
                if finished && next.is_none() && current.is_none() && inbuf.is_empty() {
                    // Some rz builds close without the final OO; nothing more to wait for.
                }
                if !inbuf.is_empty() {
                    let n = engine.submit_wire(&inbuf).map_err(|e| format!("{e:?}"))?;
                    inbuf.drain(..n);
                    if n > 0 {
                        continue;
                    }
                }
                if next.is_some() {
                    // Waiting for backpressure to clear; poll again after the wire drains.
                    tokio::task::yield_now().await;
                    continue;
                }
                tokio::select! {
                    _ = cancel.cancelled() => {
                        engine.abort();
                        drain_wire(pending_wire_send(&mut engine), &term).await;
                        return Err("cancelled".into());
                    }
                    chunk = rx.recv() => match chunk {
                        Some(b) => inbuf.extend_from_slice(&b),
                        None => return Err("session closed".into()),
                    },
                    _ = tokio::time::sleep(RESPONSE_TIMEOUT) => {
                        engine.timeout().map_err(|e| format!("{e:?}"))?;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Everything the engine still wants on the wire after an abort.
async fn drain_wire(chunks: Vec<Vec<u8>>, term: &SharedTerminal) {
    for v in chunks {
        if term.write(&v).await.is_err() {
            break;
        }
    }
}

fn pending_wire_recv(engine: &mut zmodem2::Receiver) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for _ in 0..8 {
        let v = match engine.poll() {
            Action::WriteWire(b) => b.to_vec(),
            _ => break,
        };
        engine.wire_written(v.len());
        out.push(v);
    }
    out
}

fn pending_wire_send(engine: &mut zmodem2::Sender) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for _ in 0..8 {
        let v = match engine.poll() {
            Action::WriteWire(b) => b.to_vec(),
            _ => break,
        };
        engine.wire_written(v.len());
        out.push(v);
    }
    out
}

fn open_for_send(path: &Path) -> std::result::Result<(std::fs::File, String, u64), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "file has no name".to_string())?;
    Ok((file, name, size))
}

/// The name `sz` announced, reduced to a safe base name.
fn receive_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let base = s.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.chars().take(200).collect()
    }
}

/// `name`, or `name (1)` … when it already exists.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1..1000 {
        let p = dir.join(format!("{stem} ({n}){ext}"));
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem} ({}){ext}", Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_are_found_with_their_direction() {
        assert_eq!(
            find_header(b"rz\r**\x18B00000000000000\r\x8a\x11"),
            Some((3, Kind::Receive))
        );
        assert_eq!(
            find_header(b"**\x18B0100000023be50\r\x8a\x11"),
            Some((0, Kind::Send))
        );
        assert_eq!(find_header(b"\x18B01"), Some((0, Kind::Send)));
        assert_eq!(find_header(b"hello **B00"), None);
        assert_eq!(find_header(b"ls -la\r\n"), None);
    }

    #[test]
    fn names_are_sanitised_and_unique() {
        assert_eq!(receive_name(b"/tmp/dir/report.pdf"), "report.pdf");
        assert_eq!(receive_name(b"C:\\x\\a:b.txt"), "a_b.txt");
        assert_eq!(receive_name(b"..."), "file");
        assert_eq!(receive_name(b""), "file");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        assert_eq!(
            unique_path(dir.path(), "a.txt"),
            dir.path().join("a (1).txt")
        );
        assert_eq!(unique_path(dir.path(), "b.txt"), dir.path().join("b.txt"));
    }

    #[test]
    fn abort_sequence_is_can_then_bs() {
        let a = abort_sequence();
        assert_eq!(a.len(), 16);
        assert!(a[..8].iter().all(|&b| b == 0x18));
        assert!(a[8..].iter().all(|&b| b == 0x08));
    }
}

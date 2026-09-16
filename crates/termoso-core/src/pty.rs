//! Local terminal: a shell in a PTY via `portable-pty`, exposed through
//! [`TerminalSession`] like every other terminal.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::sync::Mutex;

use crate::error::{CoreError, Result};
use crate::terminal::{TermEvent, TermEvents, TermSize, TerminalSession, event_channel};

/// What to launch.
#[derive(Debug, Clone, Default)]
pub struct LocalShellOptions {
    /// Program and arguments; empty = the user's default shell.
    pub argv: Vec<String>,
    /// Working directory.
    pub cwd: Option<std::path::PathBuf>,
    /// Extra environment.
    pub env: Vec<(String, String)>,
    /// Initial size.
    pub size: TermSize,
}

/// A running local shell.
pub struct LocalTerminal {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: std::sync::Mutex<Box<dyn Write + Send>>,
    killer: std::sync::Mutex<Box<dyn ChildKiller + Send + Sync>>,
    closed: AtomicBool,
    pid: Option<u32>,
}

impl std::fmt::Debug for LocalTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalTerminal")
            .field("pid", &self.pid)
            .finish()
    }
}

fn size(s: TermSize) -> PtySize {
    PtySize {
        rows: s.rows,
        cols: s.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// `$SHELL` as a login shell on macOS (Terminal.app / iTerm do the same, and
/// `~/.zprofile` is where Homebrew puts its PATH); the plain default elsewhere.
fn default_shell_command() -> CommandBuilder {
    let shell = std::env::var_os("SHELL").filter(|s| !s.is_empty());
    if let (true, Some(shell)) = (cfg!(target_os = "macos"), shell) {
        let mut cmd = CommandBuilder::new(shell);
        cmd.arg("-l");
        return cmd;
    }
    CommandBuilder::new_default_prog()
}

/// A user-picked shell, run the way Terminal.app would: a bare POSIX shell on
/// macOS gets `-l`; explicit arguments are always taken as given.
fn shell_command(argv: &[String]) -> CommandBuilder {
    let mut cmd = CommandBuilder::from_argv(argv.iter().map(Into::into).collect());
    if cfg!(target_os = "macos") && argv.len() == 1 && is_login_capable_shell(&argv[0]) {
        cmd.arg("-l");
    }
    cmd
}

fn is_login_capable_shell(program: &str) -> bool {
    matches!(
        program.rsplit('/').next().unwrap_or(program),
        "zsh" | "bash" | "sh" | "fish" | "tcsh" | "csh" | "ksh" | "dash"
    )
}

/// Base name of the shell [`LocalTerminal::spawn`] starts when `argv` is
/// empty (`$SHELL` on Unix, `cmd` on Windows).
pub fn default_shell_name() -> Option<String> {
    if cfg!(windows) {
        return Some("cmd".into());
    }
    let shell = std::env::var("SHELL").ok()?;
    let name = shell.rsplit('/').next()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

impl LocalTerminal {
    /// Spawn the shell.
    pub fn spawn(opts: LocalShellOptions) -> Result<(Arc<LocalTerminal>, TermEvents)> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(size(opts.size))
            .map_err(|e| CoreError::Terminal(e.to_string()))?;

        let mut cmd = if opts.argv.is_empty() {
            default_shell_command()
        } else {
            shell_command(&opts.argv)
        };
        if let Some(cwd) = &opts.cwd {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "termoso");
        // GUI apps on macOS inherit no locale; without one the shell (and every
        // ssh session started from it) falls back to ASCII.
        if cfg!(target_os = "macos") && std::env::var_os("LANG").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        for (k, v) in &opts.env {
            cmd.env(k, v);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| CoreError::Terminal(e.to_string()))?;
        drop(pair.slave);

        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| CoreError::Terminal(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| CoreError::Terminal(e.to_string()))?;

        let term = Arc::new(LocalTerminal {
            master: Mutex::new(pair.master),
            writer: std::sync::Mutex::new(writer),
            killer: std::sync::Mutex::new(killer),
            closed: AtomicBool::new(false),
            pid,
        });

        let (tx, rx) = event_channel();
        let t2 = term.clone();
        // Blocking reader thread; PTY reads have no async API on all platforms.
        std::thread::Builder::new()
            .name("termoso-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let chunk = bytes::Bytes::copy_from_slice(&buf[..n]);
                            if tx.blocking_send(TermEvent::Output(chunk)).is_err() {
                                break;
                            }
                        }
                    }
                }
                t2.closed.store(true, Ordering::SeqCst);
                let status = child.wait().ok();
                let event = match status {
                    Some(s) => TermEvent::Exit {
                        code: Some(s.exit_code()),
                        signal: s.signal().map(str::to_string),
                    },
                    None => TermEvent::Closed,
                };
                let _ = tx.blocking_send(event);
            })
            .map_err(|e| CoreError::Terminal(e.to_string()))?;

        Ok((term, rx))
    }

    /// Child process id.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }
}

#[async_trait]
impl TerminalSession for LocalTerminal {
    async fn write(&self, data: &[u8]) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CoreError::Closed);
        }
        let mut w = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    async fn resize(&self, s: TermSize) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.master
            .lock()
            .await
            .resize(size(s))
            .map_err(|e| CoreError::Terminal(e.to_string()))
    }

    async fn close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let _ = self.killer.lock().unwrap_or_else(|p| p.into_inner()).kill();
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "local"
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn login_shell_detection() {
        assert!(is_login_capable_shell("/bin/zsh"));
        assert!(is_login_capable_shell("/opt/homebrew/bin/fish"));
        assert!(is_login_capable_shell("bash"));
        assert!(!is_login_capable_shell("/usr/bin/python3"));
        assert!(!is_login_capable_shell("wsl.exe -d Ubuntu"));
    }

    #[tokio::test]
    async fn local_shell_echoes_and_exits() {
        let (term, mut events) = LocalTerminal::spawn(LocalShellOptions {
            argv: vec!["/bin/sh".into()],
            ..Default::default()
        })
        .unwrap();
        term.write(b"echo termoso-ok; exit 3\n").await.unwrap();
        let mut out = Vec::new();
        let mut exit = None;
        while let Some(ev) = tokio::time::timeout(std::time::Duration::from_secs(10), events.recv())
            .await
            .unwrap()
        {
            match ev {
                TermEvent::Output(b) => out.extend_from_slice(&b),
                TermEvent::Exit { code, .. } => {
                    exit = code;
                    break;
                }
                TermEvent::Closed => break,
                _ => {}
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("termoso-ok"));
        assert_eq!(exit, Some(3));
        assert!(term.write(b"x").await.is_err());
    }
}

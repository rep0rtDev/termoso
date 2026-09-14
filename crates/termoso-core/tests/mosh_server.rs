//! The built-in Mosh client against a stock `mosh-server` on loopback: the
//! whole wire stack (OCB, fragments, protobuf, state sync) has to line up
//! with the reference implementation for a single byte to get through.
//!
//! Skips when `mosh-server` is not installed (set `TERMOSO_TEST_REQUIRE_MOSH=1`
//! to fail instead).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use termoso_core::mosh;
use termoso_core::terminal::{TermEvent, TermEvents, TermSize};

struct Server {
    /// The detached server process (the one we spawned only prints the
    /// coordinates and exits).
    pid: u32,
    boot: mosh::Bootstrap,
}

impl Server {
    fn running(&self) -> bool {
        std::path::Path::new(&format!("/proc/{}", self.pid)).exists()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .arg(self.pid.to_string())
            .stderr(Stdio::null())
            .status();
    }
}

fn spawn_server(shell: &str) -> Option<Server> {
    let which = Command::new("sh")
        .args(["-c", "command -v mosh-server"])
        .output()
        .ok()?;
    if !which.status.success() {
        if std::env::var("TERMOSO_TEST_REQUIRE_MOSH").is_ok() {
            panic!("mosh-server not installed but TERMOSO_TEST_REQUIRE_MOSH is set");
        }
        eprintln!("mosh-server not installed; skipping");
        return None;
    }
    let child = Command::new("mosh-server")
        .args([
            "new",
            "-s",
            "-i",
            "127.0.0.1",
            "-c",
            "256",
            "-l",
            "LANG=C.UTF-8",
        ])
        .args(["--", "sh", "-c", shell])
        .env("SSH_CONNECTION", "127.0.0.1 1 127.0.0.1 22")
        .env("TERM", "xterm-256color")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mosh-server");
    let out = child.wait_with_output().expect("mosh-server output");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let boot = mosh::parse_server_output(&stdout).unwrap_or_else(|| {
        panic!("no MOSH CONNECT from mosh-server\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });
    let pid = stdout
        .lines()
        .chain(stderr.lines())
        .find_map(|l| {
            l.strip_prefix("[mosh-server detached, pid = ")
                .and_then(|r| r.trim_end_matches(']').trim().parse::<u32>().ok())
        })
        .unwrap_or_else(|| panic!("no detached pid\nstdout:\n{stdout}\nstderr:\n{stderr}"));
    Some(Server { pid, boot })
}

async fn wait_for(events: &mut TermEvents, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(
            !left.is_zero(),
            "timed out waiting for {needle:?}; got:\n{}",
            String::from_utf8_lossy(&seen)
        );
        match tokio::time::timeout(left, events.recv()).await {
            Ok(Some(TermEvent::Output(b))) => {
                seen.extend_from_slice(&b);
                let text = String::from_utf8_lossy(&seen).into_owned();
                if text.contains(needle) {
                    return text;
                }
            }
            Ok(Some(TermEvent::Notice(_))) => {}
            Ok(Some(other)) => panic!("unexpected event {other:?} while waiting for {needle:?}"),
            Ok(None) => panic!("event channel closed while waiting for {needle:?}"),
            Err(_) => {}
        }
    }
}

#[tokio::test]
async fn keystrokes_resize_and_clean_shutdown_against_real_mosh_server() {
    let Some(server) = spawn_server(
        "stty -echo; printf 'READY_%s\\n' MARK; while IFS= read -r l; do eval \"$l\"; done",
    ) else {
        return;
    };
    let size = TermSize {
        cols: 100,
        rows: 30,
    };
    let (terminal, mut events) = mosh::connect("127.0.0.1", &server.boot, size)
        .await
        .expect("udp session");
    use termoso_core::terminal::TerminalSession;
    assert_eq!(terminal.kind(), "mosh");

    wait_for(&mut events, "READY_MARK", Duration::from_secs(10)).await;

    // Input goes through: the shell evaluates what we type and prints a
    // marker that cannot come from an echo of the keystrokes.
    terminal
        .write(b"printf '%s%s\\n' PONG _OK\n")
        .await
        .expect("write");
    wait_for(&mut events, "PONG_OK", Duration::from_secs(10)).await;

    // Resize reaches the pty.
    terminal
        .resize(TermSize {
            cols: 132,
            rows: 43,
        })
        .await
        .expect("resize");
    terminal
        .write(b"printf 'SIZE=%s\\n' \"$(stty size)\"\n")
        .await
        .expect("write");
    wait_for(&mut events, "SIZE=43 132", Duration::from_secs(10)).await;

    // A burst of small writes stays in order.
    for ch in [
        "printf ",
        "'%s' ",
        "A",
        "B",
        "C",
        "D",
        "E",
        "F",
        "; echo _END",
        "\n",
    ] {
        terminal.write(ch.as_bytes()).await.expect("write");
    }
    wait_for(&mut events, "ABCDEF_END", Duration::from_secs(10)).await;

    // Clean shutdown: the server acknowledges and exits on its own.
    terminal.close().await.expect("close");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match tokio::time::timeout(Duration::from_secs(10), events.recv()).await {
            Ok(Some(TermEvent::Closed)) | Ok(Some(TermEvent::Exit { .. })) | Ok(None) => break,
            Ok(Some(TermEvent::Error(e))) => panic!("error on close: {e}"),
            Ok(Some(_)) => {}
            Err(_) => panic!("no close event"),
        }
    }
    while server.running() {
        assert!(
            Instant::now() < deadline,
            "mosh-server still running after clean shutdown"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A UDP relay that drops every third datagram in each direction and
/// reorders the rest in pairs; returns the address the client should use.
async fn lossy_relay(server: std::net::SocketAddr) -> std::net::SocketAddr {
    use tokio::net::UdpSocket;
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("bind relay");
    let addr = sock.local_addr().expect("relay addr");
    tokio::spawn(async move {
        let mut client: Option<std::net::SocketAddr> = None;
        let mut buf = vec![0u8; 4096];
        let mut n_in = 0u32;
        let mut n_out = 0u32;
        let mut held: Option<(Vec<u8>, std::net::SocketAddr)> = None;
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf).await else {
                break;
            };
            let data = buf[..n].to_vec();
            let (to, counter) = if from == server {
                let Some(c) = client else { continue };
                (c, &mut n_out)
            } else {
                client = Some(from);
                (server, &mut n_in)
            };
            *counter += 1;
            if *counter % 3 == 0 {
                continue;
            }
            match held.take() {
                Some((prev, prev_to)) => {
                    let _ = sock.send_to(&data, to).await;
                    let _ = sock.send_to(&prev, prev_to).await;
                }
                None if *counter % 5 == 1 => held = Some((data, to)),
                None => {
                    let _ = sock.send_to(&data, to).await;
                }
            }
        }
    });
    addr
}

#[tokio::test]
async fn survives_loss_and_reordering() {
    let Some(server) =
        spawn_server("stty -echo; echo READY_MARK; while IFS= read -r l; do eval \"$l\"; done")
    else {
        return;
    };
    let relay = lossy_relay(std::net::SocketAddr::new(
        "127.0.0.1".parse().unwrap(),
        server.boot.port,
    ))
    .await;
    let boot = mosh::Bootstrap {
        ip: None,
        port: relay.port(),
        key: server.boot.key.clone(),
    };
    let (terminal, mut events) = mosh::connect("127.0.0.1", &boot, TermSize { cols: 80, rows: 24 })
        .await
        .expect("udp session through lossy relay");
    use termoso_core::terminal::TerminalSession;
    wait_for(&mut events, "READY_MARK", Duration::from_secs(20)).await;
    for i in 0..20 {
        terminal
            .write(format!("echo LINE_{i}_DONE\n").as_bytes())
            .await
            .expect("write");
    }
    let text = wait_for(&mut events, "LINE_19_DONE", Duration::from_secs(30)).await;
    for i in 0..20 {
        assert!(
            text.contains(&format!("LINE_{i}_DONE")),
            "missing line {i}:\n{text}"
        );
    }
    terminal.close().await.expect("close");
}

#[tokio::test]
async fn unreachable_port_fails_fast_with_a_udp_hint() {
    // Nothing listens here; the key only has to be well-formed.
    let boot = mosh::Bootstrap {
        ip: Some("127.0.0.1".into()),
        port: 1,
        key: zeroize::Zeroizing::new("AAAAAAAAAAAAAAAAAAAAAA".to_string()),
    };
    let started = Instant::now();
    let err = mosh::connect("127.0.0.1", &boot, TermSize { cols: 80, rows: 24 })
        .await
        .err()
        .expect("must fail");
    assert_eq!(err.kind(), "mosh");
    assert!(err.to_string().contains("UDP 60000–61000"), "{err}");
    assert!(started.elapsed() <= mosh::CONNECT_TIMEOUT + Duration::from_secs(2));
}

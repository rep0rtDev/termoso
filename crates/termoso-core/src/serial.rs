//! Serial console: a COM / tty device opened through `serialport`, exposed
//! through [`TerminalSession`] like every other terminal. There is no
//! remote side to negotiate with, so resize is a no-op and "exit" only
//! happens when the device goes away or the session is closed.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use encoding_rs::{Decoder, Encoder, EncoderResult, Encoding, UTF_8};
use serde::{Deserialize, Serialize};
use serialport::{DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits};

use crate::error::{CoreError, Result};
use crate::model::SerialConfig;
use crate::terminal::{TermEvent, TermEvents, TermSize, TerminalSession, event_channel};

/// Reads block for at most this long so a close request is noticed promptly.
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// Baud rates offered by the UI, most common first.
pub const COMMON_BAUD_RATES: &[u32] = &[
    115_200, 9_600, 19_200, 38_400, 57_600, 230_400, 460_800, 921_600, 1_200, 2_400, 4_800,
];

/// Default line settings: 115200 8N1, no flow control.
pub fn default_config() -> SerialConfig {
    SerialConfig {
        path: String::new(),
        baud_rate: 115_200,
        data_bits: 8,
        stop_bits: 1,
        parity: "none".into(),
        flow_control: "none".into(),
        charset: String::new(),
    }
}

/// Charsets offered by the UI as WHATWG labels (UTF-8 first); any label
/// `encoding_rs` knows is accepted.
pub const COMMON_CHARSETS: &[&str] = &[
    "utf-8",
    "iso-8859-1",
    "iso-8859-2",
    "iso-8859-15",
    "windows-1250",
    "windows-1251",
    "windows-1252",
    "koi8-r",
    "koi8-u",
    "gbk",
    "gb18030",
    "big5",
    "shift_jis",
    "euc-jp",
    "euc-kr",
];

/// Resolve a charset label. Empty means UTF-8; UTF-16 is refused because a
/// byte-oriented console cannot carry it.
fn charset(label: &str) -> Result<&'static Encoding> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(UTF_8);
    }
    match Encoding::for_label(label.as_bytes()) {
        Some(e) if e.is_ascii_compatible() => Ok(e),
        Some(e) => Err(CoreError::Invalid(format!(
            "charset {} cannot be used on a serial line",
            e.name()
        ))),
        None => Err(CoreError::Invalid(format!("unknown charset {label:?}"))),
    }
}

/// Streaming charset → UTF-8 conversion for device output. UTF-8 passes
/// through untouched so the terminal keeps its own decoder state.
struct Transcoder {
    decoder: Option<Decoder>,
}

impl Transcoder {
    fn new(enc: &'static Encoding) -> Self {
        Self {
            decoder: (enc != UTF_8).then(|| enc.new_decoder()),
        }
    }

    fn decode(&mut self, input: &[u8]) -> bytes::Bytes {
        match &mut self.decoder {
            None => bytes::Bytes::copy_from_slice(input),
            Some(d) => {
                let mut out = String::with_capacity(
                    d.max_utf8_buffer_length(input.len())
                        .unwrap_or(input.len() * 3),
                );
                let _ = d.decode_to_string(input, &mut out, false);
                bytes::Bytes::from(out)
            }
        }
    }
}

/// UTF-8 → charset for keyboard input; characters the charset lacks become `?`.
fn encode_input(encoder: &mut Option<Encoder>, text: &[u8]) -> Vec<u8> {
    let Some(enc) = encoder else {
        return text.to_vec();
    };
    let s = String::from_utf8_lossy(text);
    let mut out = Vec::with_capacity(
        enc.max_buffer_length_from_utf8_if_no_unmappables(s.len())
            .unwrap_or(s.len() * 2),
    );
    let mut rest: &str = &s;
    loop {
        let (res, read) = enc.encode_from_utf8_to_vec_without_replacement(rest, &mut out, false);
        rest = &rest[read..];
        match res {
            EncoderResult::InputEmpty => break,
            EncoderResult::OutputFull => out.reserve(rest.len() * 4 + 16),
            EncoderResult::Unmappable(_) => out.push(b'?'),
        }
    }
    out
}

/// A serial device present on this machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    /// Device path (`/dev/ttyUSB0`, `COM3`).
    pub path: String,
    /// `usb` | `pci` | `bluetooth` | `unknown`.
    pub kind: String,
    /// USB manufacturer, when the bus reports one.
    pub manufacturer: Option<String>,
    /// USB product name, when the bus reports one.
    pub product: Option<String>,
    /// USB serial number, when the bus reports one.
    pub serial_number: Option<String>,
}

/// Enumerate serial devices. Platforms without enumeration yield an empty
/// list; the user can still type a path by hand.
pub fn available_ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let (kind, usb) = match p.port_type {
                SerialPortType::UsbPort(u) => ("usb", Some(u)),
                SerialPortType::PciPort => ("pci", None),
                SerialPortType::BluetoothPort => ("bluetooth", None),
                SerialPortType::Unknown => ("unknown", None),
            };
            PortInfo {
                path: p.port_name,
                kind: kind.into(),
                manufacturer: usb.as_ref().and_then(|u| u.manufacturer.clone()),
                product: usb.as_ref().and_then(|u| u.product.clone()),
                serial_number: usb.as_ref().and_then(|u| u.serial_number.clone()),
            }
        })
        .collect();
    // USB adapters first (that is what people plug in), then by name.
    ports.sort_by(|a, b| {
        (a.kind != "usb")
            .cmp(&(b.kind != "usb"))
            .then_with(|| a.path.cmp(&b.path))
    });
    ports.dedup_by(|a, b| a.path == b.path);
    ports
}

/// Validate and translate the stored line settings into `serialport` types.
fn line_settings(cfg: &SerialConfig) -> Result<(DataBits, StopBits, Parity, FlowControl)> {
    let data_bits = match cfg.data_bits {
        0 | 8 => DataBits::Eight,
        7 => DataBits::Seven,
        6 => DataBits::Six,
        5 => DataBits::Five,
        n => {
            return Err(CoreError::Invalid(format!(
                "data bits must be 5–8, got {n}"
            )));
        }
    };
    let stop_bits = match cfg.stop_bits {
        0 | 1 => StopBits::One,
        2 => StopBits::Two,
        n => {
            return Err(CoreError::Invalid(format!(
                "stop bits must be 1 or 2, got {n}"
            )));
        }
    };
    let parity = match cfg.parity.trim().to_ascii_lowercase().as_str() {
        "" | "none" | "n" => Parity::None,
        "odd" | "o" => Parity::Odd,
        "even" | "e" => Parity::Even,
        other => return Err(CoreError::Invalid(format!("unknown parity {other:?}"))),
    };
    let flow_control = match cfg.flow_control.trim().to_ascii_lowercase().as_str() {
        "" | "none" => FlowControl::None,
        "software" | "xon/xoff" | "xonxoff" => FlowControl::Software,
        "hardware" | "rts/cts" | "rtscts" => FlowControl::Hardware,
        other => {
            return Err(CoreError::Invalid(format!(
                "unknown flow control {other:?}"
            )));
        }
    };
    Ok((data_bits, stop_bits, parity, flow_control))
}

/// Check that a config can be opened later (path present, line settings
/// valid) without touching the device.
pub fn validate(cfg: &SerialConfig) -> Result<()> {
    if cfg.path.trim().is_empty() {
        return Err(CoreError::Invalid("serial port path is required".into()));
    }
    charset(&cfg.charset)?;
    line_settings(cfg).map(|_| ())
}

/// Short human description, e.g. `115200 8N1`.
pub fn describe(cfg: &SerialConfig) -> String {
    let parity = match cfg.parity.trim().to_ascii_lowercase().as_str() {
        "odd" | "o" => 'O',
        "even" | "e" => 'E',
        _ => 'N',
    };
    let data = if cfg.data_bits == 0 { 8 } else { cfg.data_bits };
    let stop = if cfg.stop_bits == 0 { 1 } else { cfg.stop_bits };
    let baud = if cfg.baud_rate == 0 {
        115_200
    } else {
        cfg.baud_rate
    };
    format!("{baud} {data}{parity}{stop}")
}

fn map_err(path: &str, e: serialport::Error) -> CoreError {
    let msg = format!("{path}: {}", e.description);
    match e.kind {
        serialport::ErrorKind::NoDevice
        | serialport::ErrorKind::Io(std::io::ErrorKind::NotFound) => CoreError::NotFound(msg),
        serialport::ErrorKind::InvalidInput => CoreError::Invalid(msg),
        _ => CoreError::Terminal(msg),
    }
}

/// An open serial console.
pub struct SerialTerminal {
    writer: std::sync::Mutex<Box<dyn SerialPort>>,
    /// `None` when the line speaks UTF-8.
    encoder: std::sync::Mutex<Option<Encoder>>,
    closed: AtomicBool,
    path: String,
}

impl std::fmt::Debug for SerialTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTerminal")
            .field("path", &self.path)
            .finish()
    }
}

impl SerialTerminal {
    /// Open the device and start pumping its output.
    pub fn open(cfg: &SerialConfig) -> Result<(Arc<SerialTerminal>, TermEvents)> {
        let path = cfg.path.trim();
        if path.is_empty() {
            return Err(CoreError::Invalid("serial port path is required".into()));
        }
        let (data_bits, stop_bits, parity, flow_control) = line_settings(cfg)?;
        let enc = charset(&cfg.charset)?;
        let baud = if cfg.baud_rate == 0 {
            115_200
        } else {
            cfg.baud_rate
        };
        let port = serialport::new(path, baud)
            .data_bits(data_bits)
            .stop_bits(stop_bits)
            .parity(parity)
            .flow_control(flow_control)
            .timeout(READ_TIMEOUT)
            .open()
            .map_err(|e| map_err(path, e))?;
        let mut reader = port.try_clone().map_err(|e| map_err(path, e))?;

        let term = Arc::new(SerialTerminal {
            writer: std::sync::Mutex::new(port),
            encoder: std::sync::Mutex::new((enc != UTF_8).then(|| enc.new_encoder())),
            closed: AtomicBool::new(false),
            path: path.to_string(),
        });

        let (tx, rx) = event_channel();
        let t2 = term.clone();
        std::thread::Builder::new()
            .name("termoso-serial-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                let mut transcoder = Transcoder::new(enc);
                let mut error = None;
                while !t2.closed.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(0) => continue,
                        Ok(n) => {
                            let chunk = transcoder.decode(&buf[..n]);
                            if tx.blocking_send(TermEvent::Output(chunk)).is_err() {
                                break;
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::TimedOut
                                    | std::io::ErrorKind::WouldBlock
                                    | std::io::ErrorKind::Interrupted
                            ) =>
                        {
                            continue;
                        }
                        Err(e) => {
                            error = Some(format!("{}: {e}", t2.path));
                            break;
                        }
                    }
                }
                let user_closed = t2.closed.swap(true, Ordering::SeqCst);
                if let Some(message) = error
                    && !user_closed
                {
                    let _ = tx.blocking_send(TermEvent::Error(message));
                }
                let _ = tx.blocking_send(TermEvent::Closed);
            })
            .map_err(|e| CoreError::Terminal(e.to_string()))?;

        Ok((term, rx))
    }

    /// Device path this console is attached to.
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[async_trait]
impl TerminalSession for SerialTerminal {
    async fn write(&self, data: &[u8]) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CoreError::Closed);
        }
        let bytes = {
            let mut enc = self.encoder.lock().unwrap_or_else(|p| p.into_inner());
            encode_input(&mut enc, data)
        };
        let mut w = self.writer.lock().unwrap_or_else(|p| p.into_inner());
        w.write_all(&bytes)?;
        w.flush()?;
        Ok(())
    }

    async fn resize(&self, _size: TermSize) -> Result<()> {
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "serial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_settings_accept_common_spellings() {
        let mut cfg = default_config();
        cfg.parity = "Even".into();
        cfg.flow_control = "RTS/CTS".into();
        cfg.data_bits = 7;
        cfg.stop_bits = 2;
        let (d, s, p, f) = line_settings(&cfg).unwrap();
        assert_eq!(d, DataBits::Seven);
        assert_eq!(s, StopBits::Two);
        assert_eq!(p, Parity::Even);
        assert_eq!(f, FlowControl::Hardware);
        assert_eq!(describe(&cfg), "115200 7E2");
    }

    #[test]
    fn line_settings_reject_garbage() {
        let mut cfg = default_config();
        cfg.data_bits = 9;
        assert!(line_settings(&cfg).is_err());
        cfg.data_bits = 8;
        cfg.parity = "mark".into();
        assert!(line_settings(&cfg).is_err());
    }

    #[test]
    fn zero_defaults_describe_as_8n1() {
        assert_eq!(describe(&SerialConfig::default()), "115200 8N1");
    }

    #[test]
    fn missing_device_is_not_found() {
        let mut cfg = default_config();
        cfg.path = if cfg!(windows) {
            "COM255".into()
        } else {
            "/dev/termoso-no-such-tty".into()
        };
        let err = SerialTerminal::open(&cfg).unwrap_err();
        assert!(matches!(
            err,
            CoreError::NotFound(_) | CoreError::Terminal(_) | CoreError::Io(_)
        ));
    }

    /// Round-trips bytes through a `socat` pty pair; skipped when socat is
    /// not installed.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn loopback_over_socat_pty_pair() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("ttyA");
        let b = dir.path().join("ttyB");
        let Ok(mut socat) = std::process::Command::new("socat")
            .arg(format!("pty,raw,echo=0,link={}", a.display()))
            .arg(format!("pty,raw,echo=0,link={}", b.display()))
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            eprintln!("socat not available; skipping");
            return;
        };
        for _ in 0..50 {
            if a.exists() && b.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !(a.exists() && b.exists()) {
            let _ = socat.kill();
            eprintln!("socat did not create the pty pair; skipping");
            return;
        }

        let mut cfg = default_config();
        cfg.path = a.to_string_lossy().into_owned();
        let (term, mut events) = SerialTerminal::open(&cfg).unwrap();
        let mut peer = serialport::new(b.to_string_lossy(), 115_200)
            .timeout(Duration::from_secs(2))
            .open()
            .unwrap();

        term.write(b"ping\r").await.unwrap();
        let mut got = Vec::new();
        while got.len() < 5 {
            let mut buf = [0u8; 64];
            let n = peer.read(&mut buf).unwrap();
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(&got[..5], b"ping\r");

        peer.write_all(b"pong").unwrap();
        let mut out = Vec::new();
        while out.len() < 4 {
            match tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .expect("device output")
            {
                Some(TermEvent::Output(chunk)) => out.extend_from_slice(&chunk),
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert_eq!(&out[..4], b"pong");

        term.close().await.unwrap();
        assert!(matches!(term.write(b"x").await, Err(CoreError::Closed)));
        let last = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match events.recv().await {
                    Some(TermEvent::Closed) | None => break true,
                    Some(TermEvent::Error(_)) => break false,
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert!(last, "close must not surface as an error");
        let _ = socat.kill();
    }

    #[test]
    fn charset_labels() {
        assert_eq!(charset("").unwrap(), UTF_8);
        assert_eq!(charset(" UTF8 ").unwrap(), UTF_8);
        assert_eq!(charset("koi8-r").unwrap(), encoding_rs::KOI8_R);
        assert_eq!(charset("cp1251").unwrap(), encoding_rs::WINDOWS_1251);
        assert!(matches!(charset("utf-16le"), Err(CoreError::Invalid(_))));
        assert!(matches!(charset("klingon"), Err(CoreError::Invalid(_))));
        let mut cfg = default_config();
        cfg.path = "/dev/null".into();
        cfg.charset = "nope".into();
        assert!(matches!(validate(&cfg), Err(CoreError::Invalid(_))));
    }

    #[test]
    fn transcodes_both_ways() {
        // "Привет" in KOI8-R, split across two reads to exercise streaming.
        let koi8 = [0xF0u8, 0xD2, 0xC9, 0xD7, 0xC5, 0xD4];
        let mut t = Transcoder::new(encoding_rs::KOI8_R);
        let mut out = Vec::new();
        out.extend_from_slice(&t.decode(&koi8[..2]));
        out.extend_from_slice(&t.decode(&koi8[2..]));
        assert_eq!(std::str::from_utf8(&out).unwrap(), "Привет");

        let mut enc = Some(encoding_rs::KOI8_R.new_encoder());
        assert_eq!(encode_input(&mut enc, "Привет".as_bytes()), koi8);
        assert_eq!(encode_input(&mut enc, "€".as_bytes()), b"?");

        let mut none = None;
        assert_eq!(encode_input(&mut none, b"\xff"), b"\xff");
        assert_eq!(&Transcoder::new(UTF_8).decode(b"\xff")[..], b"\xff");
    }

    #[test]
    fn empty_path_is_invalid() {
        assert!(matches!(
            SerialTerminal::open(&default_config()).unwrap_err(),
            CoreError::Invalid(_)
        ));
    }
}

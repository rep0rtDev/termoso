//! Serial console over a USB-UART adapter that Kotlin drives through
//! `android.hardware.usb`. Rust only sees two byte streams: [`SerialPipe::feed`]
//! brings device output in, [`SerialLink::write`] carries keystrokes out.
//! Line settings are validated here; charset handling matches the desktop
//! serial console.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use encoding_rs::{Encoder, UTF_8};
use termoso_core::CoreError;
use termoso_core::charset::{Transcoder, charset, encode_input};
use termoso_core::terminal::{
    TermEvent, TermEvents, TermSink, TermSize, TerminalSession, event_channel,
};

use crate::error::{MobileError, Result};
use crate::session::SshSession;

/// Baud rates offered by the UI, most common first.
pub const COMMON_BAUD_RATES: &[u32] = &[
    115_200, 9_600, 19_200, 38_400, 57_600, 230_400, 460_800, 921_600, 1_200, 2_400, 4_800,
];

/// Line settings for a serial console.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SerialLine {
    pub baud_rate: u32,
    /// 5–8.
    pub data_bits: u8,
    /// 1 or 2.
    pub stop_bits: u8,
    /// `none` | `odd` | `even` | `mark` | `space`.
    pub parity: String,
    /// `none` | `hardware` (RTS/CTS) | `software` (XON/XOFF).
    pub flow_control: String,
    /// WHATWG charset label; empty = UTF-8.
    pub charset: String,
}

impl Default for SerialLine {
    fn default() -> Self {
        Self {
            baud_rate: 115_200,
            data_bits: 8,
            stop_bits: 1,
            parity: "none".into(),
            flow_control: "none".into(),
            charset: String::new(),
        }
    }
}

/// Default 115200 8N1 line.
#[uniffi::export]
pub fn default_serial_line() -> SerialLine {
    SerialLine::default()
}

/// Baud rates to offer in a picker.
#[uniffi::export]
pub fn serial_baud_rates() -> Vec<u32> {
    COMMON_BAUD_RATES.to_vec()
}

/// Reject settings the adapter drivers cannot express.
pub fn validate_line(line: &SerialLine) -> Result<()> {
    let invalid = |detail: String| MobileError::Invalid { detail };
    if line.baud_rate == 0 {
        return Err(invalid("baud rate must be positive".into()));
    }
    if !(5..=8).contains(&line.data_bits) {
        return Err(invalid(format!(
            "data bits must be 5–8, not {}",
            line.data_bits
        )));
    }
    if !matches!(line.stop_bits, 1 | 2) {
        return Err(invalid(format!(
            "stop bits must be 1 or 2, not {}",
            line.stop_bits
        )));
    }
    if !matches!(
        line.parity.as_str(),
        "none" | "odd" | "even" | "mark" | "space"
    ) {
        return Err(invalid(format!("unknown parity {:?}", line.parity)));
    }
    if !matches!(line.flow_control.as_str(), "none" | "hardware" | "software") {
        return Err(invalid(format!(
            "unknown flow control {:?}",
            line.flow_control
        )));
    }
    charset(&line.charset).map_err(|e| invalid(e.to_string()))?;
    Ok(())
}

/// The adapter side, implemented in Kotlin. Called from Rust worker threads.
#[uniffi::export(with_foreign)]
pub trait SerialLink: Send + Sync {
    /// Bytes to put on the wire (already in the line's charset).
    fn write(&self, data: Vec<u8>);
    /// The session ended; release the USB device. (Named so it does not
    /// clash with the `close()` uniffi gives every Kotlin handle.)
    fn release(&self);
}

/// A terminal whose far side is a [`SerialLink`].
pub struct PipeTerminal {
    link: Arc<dyn SerialLink>,
    sink: TermSink,
    decoder: Mutex<Transcoder>,
    encoder: Mutex<Option<Encoder>>,
    closed: AtomicBool,
}

impl PipeTerminal {
    /// Validate the line and set up the byte streams.
    pub fn open(line: &SerialLine, link: Arc<dyn SerialLink>) -> Result<(Arc<Self>, TermEvents)> {
        validate_line(line)?;
        let enc = charset(&line.charset).map_err(|e| MobileError::Invalid {
            detail: e.to_string(),
        })?;
        let (sink, events) = event_channel();
        let terminal = Arc::new(Self {
            link,
            sink,
            decoder: Mutex::new(Transcoder::new(enc)),
            encoder: Mutex::new((enc != UTF_8).then(|| enc.new_encoder())),
            closed: AtomicBool::new(false),
        });
        Ok((terminal, events))
    }

    /// Device output, called from the USB reader thread.
    fn feed(&self, data: &[u8]) {
        if data.is_empty() || self.closed.load(Ordering::Relaxed) {
            return;
        }
        let bytes = self.decoder.lock().expect("decoder poisoned").decode(data);
        if !bytes.is_empty() {
            let _ = self.sink.blocking_send(TermEvent::Output(bytes));
        }
    }

    /// The adapter went away (or failed); ends the session once.
    fn finish(&self, error: Option<String>) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let event = match error {
            Some(e) => TermEvent::Error(e),
            None => TermEvent::Closed,
        };
        let _ = self.sink.blocking_send(event);
    }
}

#[async_trait]
impl TerminalSession for PipeTerminal {
    async fn write(&self, data: &[u8]) -> std::result::Result<(), CoreError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(CoreError::Invalid("serial console closed".into()));
        }
        let bytes = encode_input(&mut self.encoder.lock().expect("encoder poisoned"), data);
        self.link.write(bytes);
        Ok(())
    }

    /// There is no far side to tell.
    async fn resize(&self, _size: TermSize) -> std::result::Result<(), CoreError> {
        Ok(())
    }

    async fn close(&self) -> std::result::Result<(), CoreError> {
        if !self.closed.swap(true, Ordering::SeqCst) {
            self.link.release();
            let _ = self.sink.try_send(TermEvent::Closed);
        }
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "serial"
    }
}

/// Kotlin's handle on the device side of a serial terminal.
#[derive(uniffi::Object)]
pub struct SerialPipe {
    terminal: Arc<PipeTerminal>,
}

impl SerialPipe {
    pub(crate) fn new(terminal: Arc<PipeTerminal>) -> Self {
        Self { terminal }
    }
}

#[uniffi::export]
impl SerialPipe {
    /// Bytes read from the adapter.
    pub fn feed(&self, data: Vec<u8>) {
        self.terminal.feed(&data);
    }

    /// The adapter was detached or failed; `error` is shown to the user.
    pub fn closed(&self, error: Option<String>) {
        self.terminal.finish(error);
    }
}

/// What [`crate::app::App::connect_serial`] hands back.
#[derive(uniffi::Record)]
pub struct SerialSession {
    pub session: Arc<SshSession>,
    pub pipe: Arc<SerialPipe>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_line_settings() {
        assert!(validate_line(&SerialLine::default()).is_ok());
        let bad = |f: fn(&mut SerialLine)| {
            let mut l = SerialLine::default();
            f(&mut l);
            validate_line(&l).is_err()
        };
        assert!(bad(|l| l.baud_rate = 0));
        assert!(bad(|l| l.data_bits = 9));
        assert!(bad(|l| l.stop_bits = 3));
        assert!(bad(|l| l.parity = "both".into()));
        assert!(bad(|l| l.flow_control = "rts".into()));
        assert!(bad(|l| l.charset = "utf-16le".into()));
        let ok = SerialLine {
            charset: "koi8-r".into(),
            ..SerialLine::default()
        };
        assert!(validate_line(&ok).is_ok());
    }
}

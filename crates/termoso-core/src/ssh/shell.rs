//! Interactive shell over an SSH channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use russh::client::Msg;
use russh::{Channel, ChannelMsg, ChannelWriteHalf};

use crate::error::Result;
use crate::terminal::{TermEvent, TermEvents, TermSize, TerminalSession, event_channel};

/// A running remote shell.
pub struct SshTerminal {
    writer: ChannelWriteHalf<Msg>,
    closed: AtomicBool,
}

pub(super) fn spawn(
    channel: Channel<Msg>,
    banner: Option<String>,
) -> (Arc<SshTerminal>, TermEvents) {
    let (tx, rx) = event_channel();
    let (mut reader, writer) = channel.split();
    let term = Arc::new(SshTerminal {
        writer,
        closed: AtomicBool::new(false),
    });
    let t2 = term.clone();
    tokio::spawn(async move {
        if let Some(b) = banner {
            let _ = tx.send(TermEvent::Notice(b)).await;
        }
        let mut exit: Option<(Option<u32>, Option<String>)> = None;
        while let Some(msg) = reader.wait().await {
            match msg {
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    if tx.send(TermEvent::Output(data)).await.is_err() {
                        break;
                    }
                }
                ChannelMsg::ExitStatus { exit_status } => {
                    exit = Some((Some(exit_status), None));
                }
                ChannelMsg::ExitSignal { signal_name, .. } => {
                    exit = Some((None, Some(format!("{signal_name:?}"))));
                }
                ChannelMsg::Eof => {}
                ChannelMsg::Close => break,
                _ => {}
            }
        }
        t2.closed.store(true, Ordering::SeqCst);
        match exit {
            Some((code, signal)) => {
                let _ = tx.send(TermEvent::Exit { code, signal }).await;
            }
            None => {
                let _ = tx.send(TermEvent::Closed).await;
            }
        }
    });
    (term, rx)
}

#[async_trait]
impl TerminalSession for SshTerminal {
    async fn write(&self, data: &[u8]) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(crate::error::CoreError::Closed);
        }
        self.writer
            .data_bytes(bytes::Bytes::copy_from_slice(data))
            .await?;
        Ok(())
    }

    async fn resize(&self, size: TermSize) -> Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.writer
            .window_change(size.cols as u32, size.rows as u32, 0, 0)
            .await?;
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        if self.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let _ = self.writer.eof().await;
        let _ = self.writer.close().await;
        Ok(())
    }

    fn kind(&self) -> &'static str {
        "ssh"
    }
}

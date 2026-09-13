//! Stream wrapper that records the server's SSH identification line.
//!
//! `russh` consumes the `SSH-2.0-...` banner internally without exposing it,
//! yet it is the cheapest OS hint there is (`OpenSSH_9.6p1 Ubuntu-3ubuntu13`,
//! `ROSSSH`, `OpenSSH_for_Windows_9.5`). The wrapper copies bytes until the
//! first line that starts with `SSH-` is complete, then becomes a plain
//! pass-through.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Shared slot the identification string lands in.
pub(crate) type ServerId = Arc<Mutex<Option<String>>>;

const MAX_PREAMBLE: usize = 4096;

pub(crate) struct IdPeek<S> {
    inner: S,
    pending: Vec<u8>,
    done: bool,
    id: ServerId,
}

impl<S> IdPeek<S> {
    pub(crate) fn new(inner: S, id: ServerId) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            done: false,
            id,
        }
    }

    fn observe(&mut self, bytes: &[u8]) {
        if self.done {
            return;
        }
        self.pending.extend_from_slice(bytes);
        // RFC 4253 allows non-`SSH-` lines before the identification string.
        while let Some(nl) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=nl).collect();
            let text = String::from_utf8_lossy(&line).trim().to_string();
            if text.starts_with("SSH-") {
                *self.id.lock().unwrap_or_else(|p| p.into_inner()) = Some(text);
                self.done = true;
                self.pending = Vec::new();
                return;
            }
        }
        if self.pending.len() > MAX_PREAMBLE {
            self.done = true;
            self.pending = Vec::new();
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for IdPeek<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let res = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &res
            && !self.done
        {
            let fresh = buf.filled()[before..].to_vec();
            self.observe(&fresh);
        }
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for IdPeek<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn records_identification_line_and_passes_bytes_through() {
        let data = b"welcome\r\nSSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13\r\nbinary\x00rest";
        let id: ServerId = Default::default();
        let mut peek = IdPeek::new(&data[..], id.clone());
        let mut out = Vec::new();
        peek.read_to_end(&mut out).await.unwrap();
        assert_eq!(out, data);
        assert_eq!(
            id.lock().unwrap().as_deref(),
            Some("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13")
        );
    }

    #[tokio::test]
    async fn gives_up_after_preamble_limit() {
        let data = vec![b'x'; MAX_PREAMBLE + 10];
        let id: ServerId = Default::default();
        let mut peek = IdPeek::new(&data[..], id.clone());
        let mut out = Vec::new();
        peek.read_to_end(&mut out).await.unwrap();
        assert_eq!(out.len(), data.len());
        assert!(id.lock().unwrap().is_none());
        assert!(peek.done);
    }
}

//! CTAP2 over ISO 7816-4 APDUs (CTAP 2.1 §11.3) – NFC tokens through the
//! platform's ISO-DEP tag API (`android.nfc.tech.IsoDep`).

use std::thread::sleep;
use std::time::{Duration, Instant};

use super::Fido2Error;
use super::ctap::CtapTransport;

/// FIDO applet AID (`A0000006472F0001`).
pub const FIDO_AID: [u8; 8] = [0xA0, 0x00, 0x00, 0x06, 0x47, 0x2F, 0x00, 0x01];

const CLA: u8 = 0x80;
const CLA_CHAINED: u8 = 0x90;
const INS_MSG: u8 = 0x10;
const INS_GET_RESPONSE_CTAP: u8 = 0x11;
const SW_OK: u16 = 0x9000;
const SW_PROCESSING: u16 = 0x9100;
const SHORT_MAX: usize = 255;

/// One APDU exchange with the tag. `transceive` sends a command APDU and
/// returns the response including the trailing `SW1 SW2`.
pub trait Apdu: Send {
    /// Command → response (data ‖ SW1 SW2).
    fn transceive(&mut self, apdu: &[u8]) -> Result<Vec<u8>, Fido2Error>;
    /// Whether the tag accepts extended-length APDUs (`IsoDep.isExtendedLengthApduSupported`).
    fn extended_length(&self) -> bool {
        false
    }
}

/// CTAP over APDUs on top of [`Apdu`].
pub struct NfcTransport<A: Apdu> {
    io: A,
    /// How long to keep polling while the token reports "processing".
    pub user_timeout: Duration,
}

impl<A: Apdu> NfcTransport<A> {
    /// Select the FIDO applet.
    pub fn open(mut io: A) -> Result<Self, Fido2Error> {
        let mut select = vec![0x00, 0xA4, 0x04, 0x00, FIDO_AID.len() as u8];
        select.extend_from_slice(&FIDO_AID);
        select.push(0x00);
        let (data, sw) = split_sw(&io.transceive(&select)?)?;
        if sw != SW_OK {
            return Err(Fido2Error::Unsupported(format!(
                "not a FIDO tag (SELECT → 0x{sw:04x})"
            )));
        }
        let version = String::from_utf8_lossy(&data);
        if !version.starts_with("FIDO_2_0") && !version.starts_with("U2F_V2") {
            return Err(Fido2Error::Unsupported(format!(
                "unknown FIDO applet version {version:?}"
            )));
        }
        Ok(Self {
            io,
            user_timeout: Duration::from_secs(60),
        })
    }

    /// Give the tag I/O back.
    pub fn into_inner(self) -> A {
        self.io
    }

    fn exchange(&mut self, payload: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        let deadline = Instant::now() + self.user_timeout;
        let first = if self.io.extended_length() || payload.len() <= SHORT_MAX {
            if self.io.extended_length() && payload.len() > SHORT_MAX {
                let mut apdu = vec![CLA, INS_MSG, 0x00, 0x00, 0x00];
                apdu.extend_from_slice(&(payload.len() as u16).to_be_bytes());
                apdu.extend_from_slice(payload);
                apdu.extend_from_slice(&[0x00, 0x00]);
                self.io.transceive(&apdu)?
            } else {
                self.io.transceive(&short_apdu(CLA, payload))?
            }
        } else {
            // Command chaining: every chunk but the last is sent with the
            // chaining bit set and yields a bare 9000.
            let mut chunks = payload.chunks(SHORT_MAX).peekable();
            let mut last = Vec::new();
            while let Some(chunk) = chunks.next() {
                let cla = if chunks.peek().is_some() {
                    CLA_CHAINED
                } else {
                    CLA
                };
                last = self.io.transceive(&short_apdu(cla, chunk))?;
                if chunks.peek().is_some() {
                    let (_, sw) = split_sw(&last)?;
                    if sw != SW_OK {
                        return Err(Fido2Error::Other(format!(
                            "APDU chaining rejected (0x{sw:04x})"
                        )));
                    }
                }
            }
            last
        };
        self.collect(first, deadline)
    }

    /// Gather a possibly multi-part response, polling through
    /// `NFCCTAP_GETRESPONSE` while the token works.
    fn collect(&mut self, mut response: Vec<u8>, deadline: Instant) -> Result<Vec<u8>, Fido2Error> {
        let mut out = Vec::new();
        loop {
            let (data, sw) = split_sw(&response)?;
            match sw {
                SW_OK => {
                    out.extend_from_slice(&data);
                    return Ok(out);
                }
                _ if sw >> 8 == 0x61 => {
                    out.extend_from_slice(&data);
                    let le = (sw & 0xFF) as u8;
                    response = self.io.transceive(&[0x00, 0xC0, 0x00, 0x00, le])?;
                }
                SW_PROCESSING => {
                    if Instant::now() >= deadline {
                        return Err(Fido2Error::Timeout);
                    }
                    sleep(Duration::from_millis(100));
                    response =
                        self.io
                            .transceive(&[CLA, INS_GET_RESPONSE_CTAP, 0x00, 0x00, 0x00])?;
                }
                0x6F00 => return Err(Fido2Error::Other("token reported a failure".into())),
                other => {
                    return Err(Fido2Error::Other(format!(
                        "unexpected APDU status 0x{other:04x}"
                    )));
                }
            }
        }
    }
}

impl<A: Apdu> CtapTransport for NfcTransport<A> {
    fn cbor(&mut self, payload: &[u8]) -> Result<Vec<u8>, Fido2Error> {
        self.exchange(payload)
    }
}

fn short_apdu(cla: u8, data: &[u8]) -> Vec<u8> {
    let mut apdu = vec![cla, INS_MSG, 0x00, 0x00, data.len() as u8];
    apdu.extend_from_slice(data);
    apdu.push(0x00);
    apdu
}

fn split_sw(response: &[u8]) -> Result<(Vec<u8>, u16), Fido2Error> {
    if response.len() < 2 {
        return Err(Fido2Error::Other("short APDU response".into()));
    }
    let (data, sw) = response.split_at(response.len() - 2);
    Ok((data.to_vec(), u16::from_be_bytes([sw[0], sw[1]])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_apdu_layout() {
        let a = short_apdu(CLA, &[1, 2, 3]);
        assert_eq!(a, vec![0x80, 0x10, 0, 0, 3, 1, 2, 3, 0]);
        assert_eq!(split_sw(&[0xAB, 0x90, 0x00]).unwrap(), (vec![0xAB], 0x9000));
        assert!(split_sw(&[0x90]).is_err());
    }
}

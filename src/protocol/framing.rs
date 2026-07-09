use anyhow::{Result, bail};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Cursor, Read};
use tracing::{debug, trace};

use super::cipher::ByondCipher;

/// A parsed BYOND protocol message.
#[derive(Debug)]
pub struct RawMessage {
    pub msg_type: u16,
    pub payload: Vec<u8>,
}

/// Reads BYOND messages from a TCP byte stream.
///
/// Post-handshake frames are always:
///   [u16 BE: seq] [u16 BE: type] [u16 BE: len] [payload: len bytes, encrypted]
///
/// Compressed batches (type 0xE4) contain:
///   [u32 LE: uncompressed_len] [zlib data]
/// And decompress to a sequence of unsequenced, unencrypted 4-byte-header messages.
pub struct FrameReader {
    buffer: Vec<u8>,
    cipher: Option<ByondCipher>,
    sequenced: bool,
}

impl FrameReader {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(65536),
            cipher: None,
            sequenced: false,
        }
    }

    pub fn set_cipher(&mut self, cipher: ByondCipher, sequenced: bool) {
        self.cipher = Some(cipher);
        self.sequenced = sequenced;
    }

    pub fn push_data(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn read_messages(&mut self) -> Result<Vec<RawMessage>> {
        let mut messages = Vec::new();
        self.read_messages_inner(&mut messages)?;
        Ok(messages)
    }

    fn read_messages_inner(&mut self, out: &mut Vec<RawMessage>) -> Result<()> {
        loop {
            let header_size = if self.sequenced { 6 } else { 4 };

            if self.buffer.len() < header_size {
                return Ok(());
            }

            let (msg_type, payload_len) = if self.sequenced {
                let mut header = Cursor::new(&self.buffer[..6]);
                let _seq = header.read_u16::<BigEndian>()?;
                let msg_type = header.read_u16::<BigEndian>()?;
                let payload_len = header.read_u16::<BigEndian>()? as usize;
                (msg_type, payload_len)
            } else {
                let mut header = Cursor::new(&self.buffer[..4]);
                let msg_type = header.read_u16::<BigEndian>()?;
                let payload_len = header.read_u16::<BigEndian>()? as usize;
                (msg_type, payload_len)
            };

            if msg_type >= 0x400 {
                bail!("invalid message type {:#06x} — likely parser misalignment", msg_type);
            }

            let total_len = header_size + payload_len;
            if self.buffer.len() < total_len {
                return Ok(());
            }

            let mut payload = self.buffer[header_size..total_len].to_vec();
            self.buffer.drain(..total_len);

            trace!(msg_type = msg_type, payload_len = payload.len(), sequenced = self.sequenced, "raw frame");

            if let Some(ref cipher) = self.cipher {
                cipher.decrypt(&mut payload);
            }

            match msg_type {
                0xA0 => {
                    if payload.len() < 6 {
                        bail!("extended message too short: {} bytes", payload.len());
                    }
                    let mut cur = Cursor::new(&payload);
                    let real_type = cur.read_u16::<BigEndian>()?;
                    let real_len = cur.read_u32::<BigEndian>()? as usize;

                    debug!(real_type = real_type, real_len = real_len, "extended message");

                    if real_len > 0x00FFFFFF {
                        bail!("extended message too large: {}", real_len);
                    }

                    let remaining = &payload[6..];
                    if remaining.len() >= real_len {
                        out.push(RawMessage {
                            msg_type: real_type,
                            payload: remaining[..real_len].to_vec(),
                        });
                    } else {
                        debug!(have = remaining.len(), need = real_len, "extended message incomplete, skipping");
                    }
                }
                0xA1 | 0xE4 => {
                    // Compressed data: [u32 LE: uncompressed_len] [zlib data]
                    if payload.len() < 4 {
                        bail!("compressed message too short: {} bytes", payload.len());
                    }
                    let mut cur = Cursor::new(&payload[..4]);
                    let _uncompressed_len = cur.read_u32::<LittleEndian>()?;

                    debug!(compressed_len = payload.len() - 4, uncompressed_len = _uncompressed_len, "decompressing batch");
                    let mut decoder = ZlibDecoder::new(&payload[4..]);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed)?;
                    debug!(decompressed_len = decompressed.len(), "batch decompressed");

                    // Decompressed data contains unsequenced, unencrypted messages
                    let mut sub_reader = FrameReader::new();
                    sub_reader.push_data(&decompressed);
                    sub_reader.read_messages_inner(out)?;
                }
                _ => {
                    debug!(msg_type = msg_type, payload_len = payload.len(), "message");
                    out.push(RawMessage {
                        msg_type,
                        payload,
                    });
                }
            }
        }
    }
}

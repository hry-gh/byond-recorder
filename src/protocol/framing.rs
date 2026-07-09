use anyhow::{Result, bail};
use byteorder::{BigEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Cursor, Read};

use super::cipher::ByondCipher;

/// A parsed BYOND protocol message.
#[derive(Debug)]
pub struct RawMessage {
    pub msg_type: u16,
    pub payload: Vec<u8>,
}

/// Reads BYOND messages from a TCP byte stream.
///
/// Handles the [u16 type][u16 length][payload] framing, extended messages
/// (type 0xA0 with u32 length), and compressed batches (type 0xE4).
pub struct FrameReader {
    buffer: Vec<u8>,
    cipher: Option<ByondCipher>,
}

impl FrameReader {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(65536),
            cipher: None,
        }
    }

    pub fn set_cipher(&mut self, cipher: ByondCipher) {
        self.cipher = Some(cipher);
    }

    pub fn push_data(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to parse one or more complete messages from the buffer.
    /// Returns all messages that can be fully parsed, leaving incomplete
    /// data in the buffer for the next call.
    pub fn read_messages(&mut self) -> Result<Vec<RawMessage>> {
        let mut messages = Vec::new();
        self.read_messages_inner(&mut messages)?;
        Ok(messages)
    }

    fn read_messages_inner(&mut self, out: &mut Vec<RawMessage>) -> Result<()> {
        loop {
            if self.buffer.len() < 4 {
                return Ok(());
            }

            let mut header = Cursor::new(&self.buffer[..4]);
            let msg_type = header.read_u16::<BigEndian>()?;
            let payload_len = header.read_u16::<BigEndian>()? as usize;

            let total_len = 4 + payload_len;
            if self.buffer.len() < total_len {
                return Ok(());
            }

            let mut payload = self.buffer[4..total_len].to_vec();
            self.buffer.drain(..total_len);

            if let Some(ref cipher) = self.cipher {
                cipher.decrypt(&mut payload);
            }

            match msg_type {
                0xA0 => {
                    // Extended message: payload contains [u16 real_type][u32 real_length][data]
                    if payload.len() < 6 {
                        bail!("extended message too short");
                    }
                    let mut cur = Cursor::new(&payload);
                    let real_type = cur.read_u16::<BigEndian>()?;
                    let real_len = cur.read_u32::<BigEndian>()? as usize;

                    if real_len > 0x00FFFFFF {
                        bail!("extended message too large: {}", real_len);
                    }

                    // The remaining data may need to be accumulated across multiple
                    // packets (state 2 in ReadPacket). For now, handle the simple case
                    // where everything arrived.
                    let remaining = &payload[6..];
                    if remaining.len() >= real_len {
                        out.push(RawMessage {
                            msg_type: real_type,
                            payload: remaining[..real_len].to_vec(),
                        });
                    }
                }
                0xE4 => {
                    // Compressed batch: decompress and re-parse
                    let mut decoder = ZlibDecoder::new(&payload[..]);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed)?;

                    let mut sub_reader = FrameReader::new();
                    // Compressed batches contain unencrypted messages
                    sub_reader.push_data(&decompressed);
                    sub_reader.read_messages_inner(out)?;
                }
                _ => {
                    out.push(RawMessage {
                        msg_type,
                        payload,
                    });
                }
            }
        }
    }
}

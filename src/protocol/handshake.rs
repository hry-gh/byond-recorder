use anyhow::{Result, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use super::cipher::ByondCipher;

/// Result of parsing the BYOND client↔server handshake.
/// Contains everything needed to decrypt subsequent traffic.
#[derive(Debug)]
pub struct HandshakeResult {
    pub client_version: u32,
    pub cipher: ByondCipher,
}

/// Parse the client's handshake message and the server's response to derive
/// the encryption key.
///
/// The handshake flow (from ServerLink_HandleHandshake in byondcore):
///   Client sends: [u16 type=0xFFFF][u16 len][client_version: u32][client_random: u32][client_key: u32][...]
///   Server sends: [u16 type=0xFFFF][u16 len][server_version: u32][min_version: u32][...][server_random: u32][server_key_add: u32][...]
///
/// Key derivation:
///   key = client_key + client_random * 0x10000 + client_version + server_key_add
pub fn derive_cipher_key(client_handshake: &[u8], server_handshake: &[u8]) -> Result<HandshakeResult> {
    if client_handshake.len() < 12 {
        bail!("client handshake too short: {} bytes", client_handshake.len());
    }

    let mut c = Cursor::new(client_handshake);
    let client_version = c.read_u32::<LittleEndian>()?;
    let client_random = c.read_u32::<LittleEndian>()?;
    let client_key = c.read_u32::<LittleEndian>()?;

    // Server response has variable length depending on version.
    // We need to extract the random values the server appends.
    // From the decompiled code, after writing fixed fields (version, min_version, flags),
    // the server writes several GenerateRandomUInt32 values.
    // The key addition is: key += server_key_add (the second-to-last random)
    //
    // The exact offsets depend on the server version and flags.
    // For now, we extract from known positions.
    if server_handshake.len() < 20 {
        bail!("server handshake too short: {} bytes", server_handshake.len());
    }

    let mut s = Cursor::new(server_handshake);
    let _server_version = s.read_u32::<LittleEndian>()?;
    let _min_version = s.read_u32::<LittleEndian>()?;
    let _flags1 = s.read_u8()?;
    let _flags2 = s.read_u8()?;
    let _flags3 = s.read_u8()?;

    // Server writes multiple random u32s. The one that affects the key is
    // the last random before the final checksum random.
    // This needs empirical validation against real handshakes.
    // TODO: verify exact byte offsets with packet captures
    let _server_random1 = s.read_u32::<LittleEndian>()?;
    let server_key_add = s.read_u32::<LittleEndian>()?;

    let key = client_key
        .wrapping_add(client_random.wrapping_mul(0x10000))
        .wrapping_add(client_version)
        .wrapping_add(server_key_add);

    Ok(HandshakeResult {
        client_version,
        cipher: ByondCipher::new(key),
    })
}

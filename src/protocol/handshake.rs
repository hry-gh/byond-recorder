use anyhow::{Result, bail};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use tracing::debug;

use super::cipher::ByondCipher;

#[derive(Debug)]
pub struct HandshakeResult {
    pub client_version: u32,
    pub cipher: ByondCipher,
}

/// Parse the client's handshake and the server's encrypted response to derive
/// the full encryption key.
///
/// Flow:
///   1. Client sends version, random, key contribution (plaintext)
///   2. Server derives partial_key = client_key + client_random * 0x10000 + client_version
///   3. Server encrypts its response with partial_key, then sends it
///   4. Server computes full_key = partial_key + server_key_add
///   5. Both sides now use full_key for all subsequent traffic
///
/// We observe both messages on the wire, so we:
///   1. Parse the plaintext client handshake to derive partial_key
///   2. Decrypt the server response using partial_key
///   3. Parse the decrypted response to find server_key_add
///   4. Compute full_key = partial_key + server_key_add
pub fn derive_cipher_key(client_handshake: &[u8], server_handshake: &[u8]) -> Result<HandshakeResult> {
    if client_handshake.len() < 12 {
        bail!("client handshake too short: {} bytes", client_handshake.len());
    }

    let mut c = Cursor::new(client_handshake);
    let client_version = c.read_u32::<LittleEndian>()?;
    let client_random = c.read_u32::<LittleEndian>()?;
    let client_key = c.read_u32::<LittleEndian>()?;

    let partial_key = client_key
        .wrapping_add(client_random.wrapping_mul(0x10000))
        .wrapping_add(client_version);

    debug!(
        client_version = client_version,
        client_random = client_random,
        client_key = format_args!("{:#010x}", client_key),
        partial_key = format_args!("{:#010x}", partial_key),
        "client handshake parsed"
    );

    // Decrypt the server response using partial_key
    let partial_cipher = ByondCipher::new(partial_key);
    let mut server_decrypted = server_handshake.to_vec();
    partial_cipher.decrypt(&mut server_decrypted);

    if server_decrypted.len() < 11 {
        bail!("server handshake too short after decrypt: {} bytes", server_decrypted.len());
    }

    let mut s = Cursor::new(&server_decrypted[..]);
    let server_version = s.read_u32::<LittleEndian>()?;
    let min_version = s.read_u32::<LittleEndian>()?;
    let flag1 = s.read_u8()?;
    let flag2 = s.read_u8()?;
    let flag3 = s.read_u8()?;

    debug!(
        server_version = server_version,
        min_version = min_version,
        flag1 = flag1,
        flag2 = flag2,
        flag3 = flag3,
        "server handshake header"
    );

    // After the 11-byte fixed header, the server writes random u32s:
    //   Loop 1: random u32s until (val + 0x1bd632f) & 0x4008000 == 0
    //   server_key_add: one u32
    //   Loop 2: more random u32s until (accum - 0x24c91d) & 0x402000 == 0
    //
    // We find server_key_add by replaying the loop termination condition.
    let remaining = &server_decrypted[11..];
    if remaining.len() < 8 {
        bail!("server handshake too short for key derivation");
    }

    let mut offset = 0;
    // Loop 1: find the first u32 where (val + 0x1bd632f) & 0x4008000 == 0
    loop {
        if offset + 4 > remaining.len() {
            bail!("ran out of data scanning loop 1 randoms");
        }
        let val = u32::from_le_bytes(remaining[offset..offset + 4].try_into().unwrap());
        offset += 4;
        debug!(val = format_args!("{:#010x}", val), offset = offset, "loop 1 random");
        if (val.wrapping_add(0x1bd632f)) & 0x4008000 == 0 {
            break;
        }
    }

    // Next u32 is server_key_add
    if offset + 4 > remaining.len() {
        bail!("ran out of data reading server_key_add");
    }
    let server_key_add = u32::from_le_bytes(remaining[offset..offset + 4].try_into().unwrap());

    let full_key = partial_key.wrapping_add(server_key_add);

    debug!(
        server_key_add = format_args!("{:#010x}", server_key_add),
        full_key = format_args!("{:#010x}", full_key),
        "key derivation complete"
    );

    Ok(HandshakeResult {
        client_version,
        cipher: ByondCipher::new(full_key),
    })
}

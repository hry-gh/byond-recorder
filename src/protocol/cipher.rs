/// BYOND's rolling subtract cipher.
///
/// The encrypt and decrypt operations are symmetric inverses using a 32-bit key.
/// Each byte is transformed using a rolling state value that incorporates previous
/// plaintext bytes, making it a stream cipher.
#[derive(Debug, Clone)]
pub struct ByondCipher {
    key: u32,
}

impl ByondCipher {
    pub fn new(key: u32) -> Self {
        Self { key }
    }

    /// Decrypt a buffer in-place. The last byte is a checksum and is removed.
    /// Returns None if the buffer is empty or the checksum fails.
    pub fn decrypt(&self, data: &mut Vec<u8>) -> Option<()> {
        if data.is_empty() {
            return None;
        }

        let len = data.len() - 1; // last byte is checksum
        let mut rolling: u8 = 0;

        for i in 0..len {
            let shift = rolling & 0x1f;
            let key_byte = (self.key >> shift) as u8;
            let cipher_val = key_byte.wrapping_add(rolling);
            data[i] = data[i].wrapping_sub(cipher_val);
            rolling = rolling.wrapping_add(data[i]);
        }

        data.truncate(len);
        Some(())
    }

    /// Encrypt a buffer in-place. Appends a checksum byte.
    pub fn encrypt(&self, data: &mut Vec<u8>) {
        let mut rolling: u8 = 0;

        for i in 0..data.len() {
            let shift = rolling & 0x1f;
            let key_byte = (self.key >> shift) as u8;
            let cipher_val = key_byte.wrapping_add(rolling);
            let plaintext = data[i];
            rolling = rolling.wrapping_add(plaintext);
            data[i] = plaintext.wrapping_add(cipher_val);
        }

        data.push(rolling);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = 0xDEADBEEF_u32;
        let cipher = ByondCipher::new(key);

        let original = b"Hello, BYOND!".to_vec();
        let mut data = original.clone();

        cipher.encrypt(&mut data);
        assert_ne!(data[..original.len()], original[..]);

        cipher.decrypt(&mut data).unwrap();
        assert_eq!(data, original);
    }
}

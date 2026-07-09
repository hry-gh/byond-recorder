/// BYOND's rolling subtract cipher.
///
/// Uses a 32-bit key and 16-bit state. The last byte of the encrypted data
/// is a checksum seed that initializes the high byte of the state.
#[derive(Debug, Clone)]
pub struct ByondCipher {
    key: u32,
}

impl ByondCipher {
    pub fn new(key: u32) -> Self {
        Self { key }
    }

    /// Decrypt a buffer in-place. The last byte is the checksum seed.
    /// Returns None if the buffer is too short.
    pub fn decrypt(&self, data: &mut Vec<u8>) -> Option<()> {
        if data.len() < 2 {
            return None;
        }

        let len = data.len();
        let checksum = data[len - 1];
        let mut state: u16 = (checksum as u16) << 8;

        for i in 0..len - 1 {
            let key_byte = (self.key >> (state & 0x1F)) as u8;
            let delta = key_byte.wrapping_add(state as u8);
            data[i] = data[i].wrapping_sub(delta);
            state = (state & 0xFF00) | ((state as u8).wrapping_add(data[i]) as u16);
        }

        data.truncate(len - 1);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_doesnt_panic_on_short_input() {
        let cipher = ByondCipher::new(0x12345678);
        let mut data = vec![0x42];
        assert!(cipher.decrypt(&mut data).is_none());
    }
}

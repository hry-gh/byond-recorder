use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};

/// Parse a .rsc file and return a map of resource_id → file path.
///
/// The .rsc format is a sequence of blocks:
///   block_size: u32 LE
///   block_type: u8 (0 = free, 1 = in use)
///   block_data: [u8; block_size]
///
/// Each in-use block contains an entry:
///   entry_type: u8
///   crc:        u32 LE
///   unk1:       u32 LE
///   unk2:       u32 LE
///   data_size:  u32 LE
///   filename:   null-terminated string
///   [file data]
pub fn parse_rsc(path: &Path) -> io::Result<HashMap<u32, String>> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len() as usize;
    let mut reader = BufReader::new(file);
    let mut map = HashMap::new();
    let mut offset = 0usize;
    let mut resource_id = 0u32;

    loop {
        if offset + 5 > file_len {
            break;
        }

        let block_size = match reader.read_u32::<LittleEndian>() {
            Ok(s) => s as usize,
            Err(_) => break,
        };
        let block_type = match reader.read_u8() {
            Ok(t) => t,
            Err(_) => break,
        };

        offset += 5;

        if block_size == 0 || offset + block_size > file_len {
            break;
        }

        if block_type == 1 {
            let mut block_data = vec![0u8; block_size];
            reader.read_exact(&mut block_data)?;

            if block_data.len() >= 17 {
                resource_id += 1;
                let name_start = 17;
                if let Some(null_pos) = block_data[name_start..].iter().position(|&b| b == 0) {
                    let name = String::from_utf8_lossy(&block_data[name_start..name_start + null_pos]);
                    let normalized = name.replace('\\', "/");
                    map.insert(resource_id, normalized);
                }
            }
        } else {
            let mut discard = vec![0u8; block_size];
            reader.read_exact(&mut discard)?;
        }

        offset += block_size;
    }

    Ok(map)
}

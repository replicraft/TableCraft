//! Original CAB construction, independent of cab's writer and reader metadata.
use flate2::{Compression, write::DeflateEncoder};
use std::io::Write;

pub fn cabinet(database: &[u8], mszip: bool) -> Vec<u8> {
    let mut blocks = Vec::new();
    for chunk in database.chunks(32768) {
        let compressed = if mszip {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(chunk).unwrap();
            [b"CK".as_slice(), &encoder.finish().unwrap()].concat()
        } else {
            chunk.to_vec()
        };
        blocks.extend_from_slice(&0u32.to_le_bytes()); // Optional checksum absent.
        blocks.extend_from_slice(&(compressed.len() as u16).to_le_bytes());
        blocks.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
        blocks.extend_from_slice(&compressed);
    }
    let mut bytes = vec![0; 36];
    bytes[..4].copy_from_slice(b"MSCF");
    bytes[16..20].copy_from_slice(&44u32.to_le_bytes());
    bytes[24..26].copy_from_slice(&[3, 1]);
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&1u16.to_le_bytes());
    let name = b"application.accdb\0";
    let data_offset = 44 + 16 + name.len();
    bytes.extend_from_slice(&(data_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&(database.len().div_ceil(32768) as u16).to_le_bytes());
    bytes.extend_from_slice(&u16::from(mszip).to_le_bytes());
    bytes.extend_from_slice(&(database.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(&blocks);
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
    bytes
}

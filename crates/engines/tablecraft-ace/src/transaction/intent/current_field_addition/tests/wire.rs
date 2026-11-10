//! Independent literal framing shared by feature-specific acceptance checks.
use sha2::{Digest, Sha256};

pub(super) fn literal(before: &[u8], table: &str, column: &str) -> Vec<u8> {
    let mut bytes = b"TCAC0008".to_vec();
    bytes.extend(((64 + table.len() + column.len()) as u32).to_le_bytes());
    bytes.extend(8_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    bytes.extend((table.len() as u16).to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(table.as_bytes());
    bytes.extend(column.as_bytes());
    bytes
}

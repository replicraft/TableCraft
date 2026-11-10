//! Literal inline Memo/OLE descriptors for host read-policy tests.
use super::{database, definition};
use crate::fixtures::access::data_page;

const PAGE_BYTES: usize = 4096;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;

/// One row containing a three-byte UTF-8 Memo and four opaque OLE bytes.
/// The wire Memo uses uncompressed UTF-16; no production encoder is involved.
pub fn database_with_inline_values() -> Vec<u8> {
    let mut bytes = database();
    let mut schema = definition(
        &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Blob", 11, 0, 0)],
        1,
        11 << 8,
    );
    schema[FIELD_START + FIELD_BYTES] = 12;
    schema[FIELD_START + FIELD_BYTES + 15] = 2;
    schema[FIELD_START + 2 * FIELD_BYTES + 15] = 2;
    bytes[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&schema);

    let mut row = vec![3, 0, 1, 0, 0, 0];
    row.extend(0x8000_0002_u32.to_le_bytes());
    row.extend([0; 8]);
    row.extend([0xac, 0x20]);
    let blob_start = row.len() as u16;
    row.extend(0x8000_0004_u32.to_le_bytes());
    row.extend([0; 8]);
    row.extend([0, 1, 254, 255]);
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(blob_start.to_le_bytes());
    row.extend(6_u16.to_le_bytes());
    row.extend(2_u16.to_le_bytes());
    row.push(7);
    bytes[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&data_page(10, &[(0, row)]));
    bytes
}

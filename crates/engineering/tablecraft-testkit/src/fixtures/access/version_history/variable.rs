//! Original raw stored-history records with variable identity and parent keys.
use crate::fixtures::access::{data_page, put16, version_history};

pub fn database() -> Vec<u8> {
    const PAGE: usize = 4096;
    const FIELD: usize = 20 * PAGE + 63;
    let mut bytes = version_history::database();
    put16(&mut bytes, 20 * PAGE + 43, 3);
    bytes[FIELD + 15] = 2;
    bytes[FIELD + 16] = 8;
    put16(&mut bytes, FIELD + 7, 0);
    put16(&mut bytes, FIELD + 25 + 7, 1);
    put16(&mut bytes, FIELD + 50 + 21, 0);
    bytes[FIELD + 75 + 15] = 6;
    bytes[FIELD + 75 + 16] = 4;
    put16(&mut bytes, FIELD + 75 + 7, 2);
    let rows = [
        (
            10_i32,
            2_i32,
            Some("é🦀\0older"),
            Some((-0.25_f64).to_bits()),
        ),
        (10, 9, Some("newer"), Some(0.25_f64.to_bits())),
        (20, 7, Some(""), Some(1)),
        (20, 3, None, None),
    ]
    .into_iter()
    .map(|(owner, identity, value, modified)| {
        let mut row = vec![4, 0];
        row.extend(modified.unwrap_or(0_u64).to_le_bytes());
        row.extend(owner.to_le_bytes());
        if let Some(value) = value {
            let text = value
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            row.extend((0x8000_0000 | text.len() as u32).to_le_bytes());
            row.extend([0; 8]);
            row.extend(text);
        }
        let memo_end = row.len() as u16;
        row.extend(identity.to_le_bytes());
        row.extend((row.len() as u16).to_le_bytes());
        row.extend(memo_end.to_le_bytes());
        row.extend([14, 0, 10, 0, 3, 0]);
        row.push(9 | u8::from(value.is_some()) << 1 | u8::from(modified.is_some()) << 2);
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&data_page(20, &rows));
    bytes
}

/// Original variable Date/Time member records in the known history template.
pub fn variable_modified() -> Vec<u8> {
    const PAGE: usize = 4096;
    const FIELD: usize = 63;
    let mut bytes = database();

    let template = 17 * PAGE + FIELD + 25;
    bytes[template + 15] = 2;
    put16(&mut bytes, template + 7, 1);
    put16(&mut bytes, 17 * PAGE + 43, 2);

    let modified = 20 * PAGE + FIELD + 50;
    bytes[modified + 15] = 2;
    put16(&mut bytes, modified + 7, 2);
    put16(&mut bytes, 20 * PAGE + FIELD + 75 + 7, 3);
    put16(&mut bytes, 20 * PAGE + 43, 4);

    let rows = [
        (
            10_i32,
            2_i32,
            Some("é🦀\0older"),
            Some((-0.25_f64).to_bits()),
        ),
        (10, 9, Some("newer"), Some(0.25_f64.to_bits())),
        (20, 7, Some(""), Some(1_u64)),
        (20, 3, None, None),
    ]
    .into_iter()
    .map(|(owner, identity, value, modified)| {
        let mut row = vec![4, 0];
        row.extend(owner.to_le_bytes());
        if let Some(value) = value {
            let text = value
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            row.extend((0x8000_0000 | text.len() as u32).to_le_bytes());
            row.extend([0; 8]);
            row.extend(text);
        }
        let memo_end = row.len() as u16;
        if let Some(modified) = modified {
            row.extend(modified.to_le_bytes());
        }
        let modified_end = row.len() as u16;
        row.extend(identity.to_le_bytes());
        row.extend((row.len() as u16).to_le_bytes());
        row.extend(modified_end.to_le_bytes());
        row.extend(memo_end.to_le_bytes());
        row.extend([6, 0, 2, 0, 4, 0]);
        row.push(9 | u8::from(value.is_some()) << 1 | u8::from(modified.is_some()) << 2);
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&data_page(20, &rows));
    bytes
}

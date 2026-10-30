//! Independent literal key weights and exact key/ROW ordering.
use super::{Family, Row};

pub(super) fn name_key(name: Option<&[u8]>, family: Family, descending: bool) -> Vec<u8> {
    let Some(name) = name else {
        return vec![if descending { 255 } else { 0 }];
    };
    let mut key = vec![if descending { 128 } else { 127 }];
    if matches!(family, Family::Binary) {
        let count = name.len().div_ceil(8);
        for (position, chunk) in name.chunks(8).enumerate() {
            let mut part = [if descending { 255 } else { 0 }; 9];
            for (output, &byte) in part.iter_mut().zip(chunk) {
                *output = if descending { !byte } else { byte };
            }
            part[8] = if position + 1 < count {
                9
            } else if descending {
                !(chunk.len() as u8)
            } else {
                chunk.len() as u8
            };
            key.extend(part);
        }
        return key;
    }

    // Independent tracked a/ss vectors establish these unaccented weights.
    // No production collation data, profile or encoded-row method is imported.
    let mut weights = Vec::new();
    for &byte in name {
        match (family, byte) {
            (Family::Legacy, b'a') => weights.push(0x4a),
            (Family::Legacy, b's') => weights.push(0x6b),
            (Family::Modern, b'a') => weights.extend([0x0e, 2]),
            (Family::Modern, b's') => weights.extend([0x0e, 0x91]),
            _ => panic!("closed independent name alphabet"),
        }
    }
    weights.extend([1, 0]);
    if descending {
        for byte in &mut weights {
            *byte = !*byte;
        }
        weights.push(0);
    }
    key.extend(weights);
    key
}

pub(super) fn entries(rows: &[Row], family: Family, descending: bool, tuple: bool) -> Vec<Vec<u8>> {
    let mut entries = Vec::new();
    for (ordinal, row) in rows.iter().enumerate() {
        let mut key = name_key(row.name.as_deref(), family, descending);
        if tuple {
            key.push(if descending { 127 } else { 128 });
            key.extend(
                ((row.age as u32) ^ 0x8000_0000)
                    .to_be_bytes()
                    .map(|byte| if descending { byte } else { !byte }),
            );
        }
        key.extend(((12_u32 << 8) | ordinal as u32).to_be_bytes());
        entries.push(key);
    }
    entries.sort();
    entries
}

use super::Row;
use crate::access as raw;

pub(super) fn entries(rows: &[Row], column: usize, descending: bool) -> Vec<Vec<u8>> {
    let mut result = rows
        .iter()
        .map(|row| {
            let value = match column {
                0 => Some(row.id),
                1 => row.score,
                2 => Some(row.guard),
                _ => panic!("unsupported authored column"),
            };
            let mut key = value.map_or_else(
                || vec![0],
                |value| {
                    let mut key = vec![127];
                    key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
                    key
                },
            );
            if descending {
                for byte in &mut key {
                    *byte = !*byte;
                }
            }
            let key_length = key.len();
            key.extend(row.reference.to_be_bytes());
            (key, key_length, row.reference)
        })
        .collect::<Vec<_>>();
    result.sort_by(|left, right| (&left.0[..left.1], left.2).cmp(&(&right.0[..right.1], right.2)));
    result.into_iter().map(|entry| entry.0).collect()
}

pub(super) fn physical(descending: bool) -> Vec<u8> {
    let mut bytes = vec![0; 52];
    raw::put32(&mut bytes, 0, 1923);
    for column in 0..10 {
        raw::put16(&mut bytes, 4 + column * 3, 0xffff);
    }
    raw::put16(&mut bytes, 4, 1);
    bytes[6] = u8::from(!descending);
    raw::put32(&mut bytes, 34, 24 << 8);
    raw::put32(&mut bytes, 38, 25);
    bytes[46] = 0x80;
    bytes
}

pub(super) fn logical() -> Vec<u8> {
    let mut bytes = vec![0; 28];
    raw::put32(&mut bytes, 0, 1625);
    raw::put32(&mut bytes, 4, 4);
    raw::put32(&mut bytes, 8, 2);
    raw::put32(&mut bytes, 13, u32::MAX);
    bytes
}

pub(super) fn owned_map(base: u32, members: &[u32]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    raw::put32(&mut bytes, 1, base);
    for &page in members {
        let relative = (page - base) as usize;
        assert!(relative < 512);
        bytes[5 + relative / 8] |= 1 << (relative % 8);
    }
    bytes
}

use super::{calculated_fixture, support};

pub fn database(
    code: u8,
    kind: u8,
    capacity: u16,
    compressed: bool,
    payloads: &[Option<Vec<u8>>],
) -> Vec<u8> {
    let mut bytes = calculated_fixture::database(&vec![Some(0); payloads.len()]);
    bytes[20] = code;
    let property = bytes
        .windows(9)
        .position(|part| part == [9, 0, 0, 2, 1, 0, 1, 0, 4])
        .unwrap();
    bytes[property + 8] = kind;
    let field = 10 * 4096 + 63 + 25;
    bytes[field] = kind;
    bytes[field + 16] = if compressed { 0xc1 } else { 0xc0 };
    support::put16(&mut bytes, field + 23, capacity);
    let rows = payloads
        .iter()
        .enumerate()
        .map(|(ordinal, payload)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((ordinal as i32 + 1).to_le_bytes());
            if let Some(payload) = payload {
                row.extend([0; 16]);
                row.extend((payload.len() as u32).to_le_bytes());
                row.extend(payload);
                row.extend([0; 3]);
            }
            row.extend((row.len() as u16).to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(if payload.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes
}

use super::{self as support, complex_schema as complex_fixture};

pub fn populated() -> Vec<u8> {
    let mut bytes = complex_fixture::with_mappings(vec![complex_fixture::mapping(7, 10, 20, 17)]);
    bytes[10 * 4096 + 63 + 25 + 15] = 7;
    support::put32(&mut bytes, 20 * 4096 + 16, 3);
    let rows = [(10_i32, 100_i32, 1_i32), (10, 101, 2), (20, 200, 3)]
        .into_iter()
        .map(|(key, value, id)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            for number in [key, value, id] {
                row.extend(number.to_le_bytes());
            }
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[22 * 4096..23 * 4096].copy_from_slice(&support::data_page(20, &rows));
    bytes
}

/// Existing independent duplicate/NULL membership and assigned-empty source.
/// Shared by the runtime summary and explicit contents observations.
pub fn presence_cases() -> Vec<u8> {
    const PAGE: usize = 4096;
    let mut bytes = populated();
    support::put32(&mut bytes, 10 * PAGE + 16, 4);
    let parents = [
        (1_i32, Some(10_i32)),
        (2, Some(20)),
        (3, Some(30)),
        (4, None),
    ]
    .into_iter()
    .map(|(id, key)| {
        let mut row = 2_u16.to_le_bytes().to_vec();
        row.extend(id.to_le_bytes());
        row.extend(key.unwrap_or(30).to_le_bytes());
        row.push(if key.is_some() { 3 } else { 1 });
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &parents));
    support::put32(&mut bytes, 20 * PAGE + 16, 4);
    let children = [
        (10_i32, Some(100_i32), 1_i32),
        (10, Some(100), 2),
        (10, None, 3),
        (20, Some(200), 4),
    ]
    .into_iter()
    .map(|(owner, value, id)| {
        let mut row = 3_u16.to_le_bytes().to_vec();
        row.extend(owner.to_le_bytes());
        row.extend(value.unwrap_or(101).to_le_bytes());
        row.extend(id.to_le_bytes());
        row.push(if value.is_some() { 7 } else { 5 });
        (0, row)
    })
    .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&support::data_page(20, &children));
    bytes
}

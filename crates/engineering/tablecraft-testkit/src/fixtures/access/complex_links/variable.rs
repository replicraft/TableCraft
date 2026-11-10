//! Original raw ACE child rows with variable-width Int32 reference carriers.
use crate::fixtures::access::{complex_links, data_page, put16};

pub fn populated() -> Vec<u8> {
    const PAGE: usize = 4096;
    const FIELD: usize = 20 * PAGE + 63;
    let mut bytes = complex_links::populated();
    put16(&mut bytes, 20 * PAGE + 43, 2);
    bytes[FIELD + 15] = 2;
    bytes[FIELD + 16] = 8;
    put16(&mut bytes, FIELD + 7, 0);
    put16(&mut bytes, FIELD + 25 + 21, 0);
    bytes[FIELD + 50 + 15] = 6;
    bytes[FIELD + 50 + 16] = 4;
    put16(&mut bytes, FIELD + 50 + 7, 1);
    let rows = [(10_i32, 100_i32, 1_i32), (10, 101, 2), (20, 200, 3)]
        .into_iter()
        .map(|(owner, value, identity)| {
            let mut row = vec![3, 0];
            row.extend(value.to_le_bytes());
            row.extend(owner.to_le_bytes());
            row.extend(identity.to_le_bytes());
            row.extend([14, 0, 10, 0, 6, 0, 2, 0, 7]);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[22 * PAGE..23 * PAGE].copy_from_slice(&data_page(20, &rows));
    bytes
}

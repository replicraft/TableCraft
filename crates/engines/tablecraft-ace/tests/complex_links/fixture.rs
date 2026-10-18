use super::{complex_fixture, support};

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

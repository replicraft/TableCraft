use super::{int64_index_fixture, support};

// Independent synthetic money key geometry; no native Access provenance claim.
pub fn database(code: u8, descending: bool, values: &[Option<i64>], prefix: usize) -> Vec<u8> {
    let mut bytes = int64_index_fixture::database(code, descending, values);
    bytes[10 * 4096 + 75 + 25] = 5; // Native MONEY; retain its eight-byte payload.
    let mut entries = values.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        let order = a.1.cmp(&b.1);
        (if descending { order.reverse() } else { order }).then(a.0.cmp(&b.0))
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page.fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    support::put16(page, 24, prefix as u16);
    let mut first = Vec::new();
    let mut end = 0;
    for (position, (ordinal, key)) in entries.into_iter().enumerate() {
        let mut encoded = vec![match (key, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(key) = key {
            let raw = ((key as u64) ^ 0x8000_0000_0000_0000).to_be_bytes();
            encoded.extend(raw.map(|byte| if descending { !byte } else { byte }));
        }
        encoded.extend(((12_u32 << 8) | ordinal as u32).to_be_bytes());
        if position == 0 {
            first = encoded.clone();
        }
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(encoded[..shared], first[..shared]);
        page[480 + end..480 + end + encoded.len() - shared].copy_from_slice(&encoded[shared..]);
        end += encoded.len() - shared;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}

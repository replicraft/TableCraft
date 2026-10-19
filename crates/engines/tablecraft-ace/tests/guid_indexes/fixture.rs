use super::{
    numeric_fixture,
    support::{data_page, put16},
};

pub fn database(code: u8, descending: bool, values: &[Option<[u8; 16]>], prefix: usize) -> Vec<u8> {
    let numeric = values
        .iter()
        .enumerate()
        .map(|(i, value)| vec![Some(i as i64 + 1), value.map(|_| 0)])
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(code, &[4, 0x13], &[(1, descending)], &numeric, 0);
    let field = 10 * 4096 + 75 + 25;
    bytes[field] = 15;
    put16(&mut bytes, field + 23, 16);
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((i as i32 + 1).to_le_bytes());
            row.extend(uuid::Uuid::from_bytes(value.unwrap_or([0; 16])).to_bytes_le());
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    let mut entries = values.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        let order = a.1.cmp(&b.1);
        (if descending { order.reverse() } else { order }).then(a.0.cmp(&b.0))
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page[24..26].copy_from_slice(&(prefix as u16).to_le_bytes());
    page[27..].fill(0);
    let mut first = Vec::new();
    let mut end = 0;
    for (i, (slot, value)) in entries.into_iter().enumerate() {
        let mut key = vec![match (value, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(value) = value {
            key.extend(value[..8].iter().map(|b| if descending { !*b } else { *b }));
            key.push(9);
            key.extend(value[8..].iter().map(|b| if descending { !*b } else { *b }));
            key.push(if descending { 0xf7 } else { 8 });
        }
        key.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
        if i == 0 {
            first = key.clone();
        }
        let shared = if i == 0 { 0 } else { prefix };
        assert_eq!(key[..shared], first[..shared]);
        page[480 + end..480 + end + key.len() - shared].copy_from_slice(&key[shared..]);
        end += key.len() - shared;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}

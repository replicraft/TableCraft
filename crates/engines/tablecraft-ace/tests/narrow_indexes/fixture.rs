use super::{int64_index_fixture, support};

pub fn database(
    code: u8,
    small: bool,
    descending: bool,
    values: &[Option<i64>],
    prefix: usize,
) -> Vec<u8> {
    let mut bytes = int64_index_fixture::database(code, descending, values);
    let field = 10 * 4096 + 75 + 25;
    bytes[field] = if small { 3 } else { 2 };
    support::put16(&mut bytes, field + 23, if small { 2 } else { 1 });
    let rows = values
        .iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let mut raw = 2_u16.to_le_bytes().to_vec();
            raw.extend((ordinal as i32 + 1).to_le_bytes());
            if small {
                raw.extend((value.unwrap_or(0) as i16).to_le_bytes());
            } else {
                raw.push(value.unwrap_or(0) as u8);
            }
            raw.push(if value.is_some() { 3 } else { 1 });
            (0, raw)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    let mut keys = values.iter().copied().enumerate().collect::<Vec<_>>();
    keys.sort_by(|a, b| {
        let order = a.1.cmp(&b.1);
        (if descending { order.reverse() } else { order }).then(a.0.cmp(&b.0))
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page.fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    support::put16(page, 24, prefix as u16);
    let mut end = 0;
    let mut first = Vec::new();
    for (position, (ordinal, value)) in keys.into_iter().enumerate() {
        let mut entry = vec![match (value, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(value) = value {
            let data = if small {
                ((value as i16 as u16) ^ 0x8000).to_be_bytes().to_vec()
            } else {
                vec![value as u8]
            };
            entry.extend(
                data.into_iter()
                    .map(|byte| if descending { !byte } else { byte }),
            );
        }
        entry.extend(((12_u32 << 8) | ordinal as u32).to_be_bytes());
        if position == 0 {
            first = entry.clone();
        }
        let shared = if position == 0 { 0 } else { prefix };
        assert_eq!(entry[..shared], first[..shared]);
        page[480 + end..480 + end + entry.len() - shared].copy_from_slice(&entry[shared..]);
        end += entry.len() - shared;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}

use super::{
    numeric_fixture,
    support::{data_page, put16},
};

// Original synthetic row/index bytes, independent of the production decimal codec.
pub fn database(
    descending: bool,
    precision: u8,
    scale: u8,
    values: &[Option<i128>],
    prefix: usize,
) -> Vec<u8> {
    database_for(6, descending, precision, scale, values, prefix)
}

pub fn database_for(
    code: u8,
    descending: bool,
    precision: u8,
    scale: u8,
    values: &[Option<i128>],
    prefix: usize,
) -> Vec<u8> {
    let proxies = values
        .iter()
        .enumerate()
        .map(|(i, value)| vec![Some(i as i64 + 1), value.map(|_| 0)])
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(code, &[4, 0x13], &[(1, descending)], &proxies, 0);
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    let field = 10 * 4096 + 75 + 25;
    bytes[field] = 16;
    bytes[field + 11] = precision;
    bytes[field + 12] = scale;
    bytes[field + 15] = 2;
    put16(&mut bytes, field + 7, 0);
    put16(&mut bytes, field + 21, 0);
    put16(&mut bytes, field + 23, 17);
    put16(&mut bytes, 10 * 4096 + 43, 1);
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend((i as i32 + 1).to_le_bytes());
            if let Some(value) = value {
                row.push(if *value < 0 { 0x80 } else { 0 });
                for word in value.unsigned_abs().to_be_bytes().as_chunks::<4>().0 {
                    row.extend(word.iter().rev());
                }
            }
            let end = row.len() as u16;
            row.extend(end.to_le_bytes());
            row.extend(6_u16.to_le_bytes());
            row.extend(1_u16.to_le_bytes());
            row.push(if value.is_some() { 3 } else { 1 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    let mut entries = values.iter().copied().enumerate().collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        let cmp = if code == 1 && descending {
            // Legacy descending groups negative coefficients before nonnegative.
            match (a.1, b.1) {
                (Some(a), Some(b)) if (a < 0) != (b < 0) => (a >= 0).cmp(&(b >= 0)).reverse(),
                _ => a.1.cmp(&b.1),
            }
        } else {
            a.1.cmp(&b.1)
        };
        (if descending { cmp.reverse() } else { cmp }).then(a.0.cmp(&b.0))
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page[27..].fill(0);
    put16(page, 24, prefix as u16);
    let mut end = 0;
    let mut first = Vec::new();
    for (i, (slot, value)) in entries.into_iter().enumerate() {
        let mut key = vec![match (value, descending) {
            (None, false) => 0,
            (None, true) => 0xff,
            (Some(_), false) => 0x7f,
            (Some(_), true) => 0x80,
        }];
        if let Some(value) = value {
            let negative = value < 0;
            let mut data = vec![if negative { 0 } else { 0xff }];
            data.extend(
                value
                    .unsigned_abs()
                    .to_be_bytes()
                    .iter()
                    .map(|b| if negative { !*b } else { *b }),
            );
            if descending {
                for byte in &mut data {
                    *byte = !*byte;
                }
                if code == 1 {
                    data[0] = if negative { 0 } else { 0xff };
                }
            }
            key.extend(data);
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

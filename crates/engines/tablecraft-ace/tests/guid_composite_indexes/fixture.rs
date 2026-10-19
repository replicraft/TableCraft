use super::{
    numeric_fixture,
    support::{data_page, put16},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Cell {
    Number(i64),
    Guid([u8; 16]),
}

pub fn database(
    code: u8,
    kinds: &[u8],
    order: &[(usize, bool)],
    rows: &[Vec<Option<Cell>>],
    prefix: usize,
) -> Vec<u8> {
    let proxies = kinds
        .iter()
        .map(|&kind| if kind == 15 { 0x13 } else { kind })
        .collect::<Vec<_>>();
    let dummy = rows
        .iter()
        .map(|row| row.iter().map(|value| value.map(|_| 0)).collect())
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(code, &proxies, order, &dummy, 0);
    let width = |kind| match kind {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 4,
        5 | 0x13 => 8,
        15 => 16,
        _ => panic!("fixture kind"),
    };
    let mut offset = 0;
    for (column, &kind) in kinds.iter().enumerate() {
        let field = 10 * 4096 + 75 + column * 25;
        bytes[field] = kind;
        put16(&mut bytes, field + 21, offset);
        put16(&mut bytes, field + 23, width(kind));
        offset += width(kind);
    }
    let raw = rows
        .iter()
        .map(|values| {
            assert_eq!(values.len(), kinds.len());
            let mut row = (kinds.len() as u16).to_le_bytes().to_vec();
            for (&kind, value) in kinds.iter().zip(values) {
                match value {
                    Some(Cell::Guid(value)) => {
                        assert_eq!(kind, 15);
                        row.extend(uuid::Uuid::from_bytes(*value).to_bytes_le());
                    }
                    Some(Cell::Number(value)) => match kind {
                        1 => assert!(matches!(value, -1 | 0)),
                        2 => row.push(u8::try_from(*value).unwrap()),
                        3 => row.extend(i16::try_from(*value).unwrap().to_le_bytes()),
                        4 => row.extend(i32::try_from(*value).unwrap().to_le_bytes()),
                        5 | 0x13 => row.extend(value.to_le_bytes()),
                        _ => panic!("numeric carrier"),
                    },
                    None => row.resize(row.len() + width(kind) as usize, 0),
                }
            }
            for (block, kinds) in values.chunks(8).zip(kinds.chunks(8)) {
                row.push(block.iter().zip(kinds).enumerate().fold(
                    0,
                    |mask, (i, (value, kind))| {
                        let bit = if *kind == 1 {
                            *value == Some(Cell::Number(-1))
                        } else {
                            value.is_some()
                        };
                        mask | u8::from(bit) << i
                    },
                ));
            }
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &raw));
    let mut entries = rows.iter().enumerate().collect::<Vec<_>>();
    entries.sort_by(|(a, x), (b, y)| {
        for &(column, descending) in order {
            let cmp = x[column].cmp(&y[column]);
            if !cmp.is_eq() {
                return if descending { cmp.reverse() } else { cmp };
            }
        }
        a.cmp(b)
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page[27..].fill(0);
    put16(page, 24, prefix as u16);
    let mut first = Vec::new();
    let mut end = 0;
    for (position, (ordinal, values)) in entries.into_iter().enumerate() {
        let mut entry = Vec::new();
        for &(column, descending) in order {
            let value = values[column];
            entry.push(match (value, descending) {
                (None, false) => 0,
                (None, true) => 0xff,
                (Some(_), false) => 0x7f,
                (Some(_), true) => 0x80,
            });
            if let Some(value) = value {
                match value {
                    Cell::Guid(value) => {
                        entry.extend(value[..8].iter().map(|b| if descending { !*b } else { *b }));
                        entry.push(9);
                        entry.extend(value[8..].iter().map(|b| if descending { !*b } else { *b }));
                        entry.push(if descending { 0xf7 } else { 8 });
                    }
                    Cell::Number(value) => {
                        let data = match kinds[column] {
                            1 => vec![if value == -1 { 0 } else { 0xff }],
                            2 => vec![value as u8],
                            3 => ((value as u16) ^ 0x8000).to_be_bytes().to_vec(),
                            4 => ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                            5 | 0x13 => {
                                ((value as u64) ^ 0x8000000000000000).to_be_bytes().to_vec()
                            }
                            _ => panic!("key kind"),
                        };
                        entry.extend(
                            data.into_iter()
                                .map(|byte| if descending { !byte } else { byte }),
                        );
                    }
                }
            }
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
    put16(page, 2, (4096 - 480 - end) as u16);
    bytes
}

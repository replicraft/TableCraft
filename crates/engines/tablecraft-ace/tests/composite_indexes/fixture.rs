use super::{
    base,
    support::{data_page, put16, put32},
};
use std::cmp::Ordering;

pub fn compare_cell(kind: u8, left: Option<i64>, right: Option<i64>) -> Ordering {
    match (left, right) {
        (Some(a), Some(b)) if kind == 6 => {
            f32::from_bits(a as u32).total_cmp(&f32::from_bits(b as u32))
        }
        (Some(a), Some(b)) if matches!(kind, 7 | 8) => {
            f64::from_bits(a as u64).total_cmp(&f64::from_bits(b as u64))
        }
        _ => left.cmp(&right),
    }
}

pub fn floating_key(kind: u8, bits: i64) -> Vec<u8> {
    match kind {
        6 => {
            let raw = bits as u32;
            (if raw & 0x80000000 == 0 {
                raw ^ 0x80000000
            } else {
                !raw
            })
            .to_be_bytes()
            .to_vec()
        }
        7 | 8 => {
            let raw = bits as u64;
            (if raw & 0x8000000000000000 == 0 {
                raw ^ 0x8000000000000000
            } else {
                !raw
            })
            .to_be_bytes()
            .to_vec()
        }
        _ => panic!("floating kind"),
    }
}

// Independent synthetic bytes. Constants here deliberately do not import the codec.
pub fn database(
    code: u8,
    kinds: &[u8],
    order: &[(usize, bool)],
    rows: &[Vec<Option<i64>>],
    prefix: usize,
) -> Vec<u8> {
    let names = [
        "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
    ];
    let width = |kind| match kind {
        1 => 0,
        2 => 1,
        3 => 2,
        4 | 6 => 4,
        5 | 7 | 8 | 0x13 => 8,
        _ => panic!("fixture type"),
    };
    let mut offset = 0;
    let fields = kinds
        .iter()
        .enumerate()
        .map(|(i, &kind)| {
            let field = (names[i], kind, offset, width(kind));
            offset += width(kind);
            field
        })
        .collect::<Vec<_>>();
    let mut definition = base::definition(&fields, rows.len() as u32, 11 << 8);
    let names_end = 63
        + kinds.len() * 25
        + names[..kinds.len()]
            .iter()
            .map(|name| 2 + name.encode_utf16().count() * 2)
            .sum::<usize>();
    definition.copy_within(63..names_end, 75);
    definition[63..75].fill(0);
    put32(&mut definition, 47, 1);
    put32(&mut definition, 51, 1);
    let at = names_end + 12;
    for i in 0..10 {
        put16(
            &mut definition,
            at + 4 + i * 3,
            order.get(i).map_or(0xffff, |key| key.0 as u16),
        );
        if let Some((_, descending)) = order.get(i) {
            definition[at + 6 + i * 3] = u8::from(!descending);
        }
    }
    put32(&mut definition, at + 34, 13 << 8);
    put32(&mut definition, at + 38, 14);
    definition[at + 46] = 0x80;
    let label = "Compound"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    put16(&mut definition, at + 80, label.len() as u16);
    definition[at + 82..at + 82 + label.len()].copy_from_slice(&label);
    let mut bytes = base::with_rule("Caption", "compound");
    bytes.resize(15 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * 4096..11 * 4096].copy_from_slice(&definition);
    let raw_rows = rows
        .iter()
        .map(|values| {
            assert_eq!(values.len(), kinds.len());
            let mut row = (kinds.len() as u16).to_le_bytes().to_vec();
            for (&kind, value) in kinds.iter().zip(values) {
                let value = value.unwrap_or(0);
                match kind {
                    1 => assert!(matches!(value, -1 | 0)),
                    2 => row.push(value as u8),
                    3 => row.extend((value as i16).to_le_bytes()),
                    4 | 6 => row.extend((value as i32).to_le_bytes()),
                    5 | 7 | 8 | 0x13 => row.extend(value.to_le_bytes()),
                    _ => unreachable!(),
                }
            }
            for (block, kinds) in values.chunks(8).zip(kinds.chunks(8)) {
                row.push(block.iter().zip(kinds).enumerate().fold(
                    0,
                    |mask, (i, (value, kind))| {
                        let bit = if *kind == 1 {
                            *value == Some(-1)
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
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &raw_rows));
    bytes[13 * 4096..14 * 4096]
        .copy_from_slice(&data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x40])]));
    let mut entries = rows.iter().enumerate().collect::<Vec<_>>();
    entries.sort_by(|(a, x), (b, y)| {
        for &(column, descending) in order {
            let cmp = compare_cell(kinds[column], x[column], y[column]);
            if !cmp.is_eq() {
                return if descending { cmp.reverse() } else { cmp };
            }
        }
        a.cmp(b)
    });
    let page = &mut bytes[14 * 4096..15 * 4096];
    page.fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    put32(page, 4, 10);
    put16(page, 24, prefix as u16);
    let mut end = 0;
    let mut first = Vec::new();
    for (position, (slot, values)) in entries.into_iter().enumerate() {
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
                let encoded = match kinds[column] {
                    1 => vec![if value == -1 { 0 } else { 0xff }],
                    2 => vec![value as u8],
                    3 => ((value as u16) ^ 0x8000).to_be_bytes().to_vec(),
                    4 => ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                    6..=8 => floating_key(kinds[column], value),
                    5 | 0x13 => ((value as u64) ^ 0x8000000000000000).to_be_bytes().to_vec(),
                    _ => unreachable!(),
                };
                entry.extend(
                    encoded
                        .into_iter()
                        .map(|byte| if descending { !byte } else { byte }),
                );
            }
        }
        entry.extend(((12_u32 << 8) | slot as u32).to_be_bytes());
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

pub fn index_flags(bytes: &mut [u8], flags: u8) {
    let column_count =
        u16::from_le_bytes(bytes[10 * 4096 + 45..10 * 4096 + 47].try_into().unwrap()) as usize;
    let names = [
        "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
    ];
    let at = 75
        + column_count * 25
        + names[..column_count]
            .iter()
            .map(|name| 2 + 2 * name.encode_utf16().count())
            .sum::<usize>();
    bytes[10 * 4096 + at + 46] = flags;
}

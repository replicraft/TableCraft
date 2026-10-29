//! Independent C4/V2 source shared by editor and CLI acceptance tests.
use super::{fixture, support};

const PAGE: usize = 4096;

fn map(members: impl IntoIterator<Item = u32>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for member in members {
        row[5 + member as usize / 8] |= 1 << (member % 8);
    }
    row
}

pub(super) fn source() -> Vec<u8> {
    let mut bytes = fixture::with_rule("Caption", "SQL engineering source");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 5;
    let mut definition = fixture::definition(
        &[
            ("Id", 4, 0, 4),
            ("Label", 10, 0, 510),
            ("Score", 4, 4, 4),
            ("Other", 10, 0, 510),
        ],
        4,
        11 << 8,
    );
    definition[63 + 25 + 15] = 2;
    definition[63 + 3 * 25 + 15] = 2;
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let mut records = Vec::new();
    for (ordinal, (label, other)) in [("A", "X"), ("B", "Y"), ("A", "Z"), ("C", "W")]
        .into_iter()
        .enumerate()
    {
        let mut row = 4_u16.to_le_bytes().to_vec();
        row.extend((ordinal as i32 + 1).to_le_bytes());
        row.extend(((ordinal as i32 + 1) * 10).to_le_bytes());
        row.extend(label.encode_utf16().flat_map(u16::to_le_bytes));
        let middle = row.len() as u16;
        row.extend(other.encode_utf16().flat_map(u16::to_le_bytes));
        row.extend((row.len() as u16).to_le_bytes());
        row.extend(middle.to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(2_u16.to_le_bytes());
        row.push(15);
        records.push((0, row));
    }
    let mut data = support::data_page(10, &records);
    support::put16(
        &mut data,
        2,
        (PAGE - 14 - records.iter().map(|(_, row)| row.len() + 2).sum::<usize>()) as u16,
    );
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&data);
    let owned = support::data_page(0, &[(0, map([12])), (0, map([12]))]);
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&owned);
    support::put32(&mut bytes, 10 * PAGE + 59, (11 << 8) | 1);
    let global = support::data_page(1, &[(0, map(14..512))]);
    bytes[PAGE..2 * PAGE].copy_from_slice(&global);
    for number in [2, 7, 10] {
        let page = &mut bytes[number * PAGE..(number + 1) * PAGE];
        let columns = usize::from(u16::from_le_bytes(page[45..47].try_into().unwrap()));
        let mut end = 63 + columns * 25;
        for _ in 0..columns {
            end += 2 + usize::from(u16::from_le_bytes(page[end..end + 2].try_into().unwrap()));
        }
        support::put16(page, end, 0xffff);
        support::put32(page, 8, (end + 2) as u32);
        support::put16(page, 2, (PAGE - end - 2 - 8) as u16);
    }
    bytes
}

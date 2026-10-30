//! Literal DATA slots, ordinary maps and stable logical-root forwarding.
use super::{Family, PAGE, Row};
use tablecraft_testkit::access as raw;

pub(super) fn data_page(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(10, rows);
    let payload = rows.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    raw::put16(&mut page, 2, (PAGE - 14 - rows.len() * 2 - payload) as u16);
    page
}

pub(super) fn record(row: &Row, family: Family) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend(row.id.to_le_bytes());
    bytes.extend(row.age.to_le_bytes());
    let payload = match &row.name {
        Some(value) if matches!(family, Family::Binary) => value.clone(),
        Some(value) => value
            .iter()
            .flat_map(|&byte| u16::from(byte).to_le_bytes())
            .collect(),
        None if matches!(family, Family::Binary) => vec![0xd3],
        None => vec![0xd3, 0x91],
    };
    bytes.extend(payload);
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend(10_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.push(if row.name.is_some() { 7 } else { 3 });
    bytes
}

pub(super) fn map_page(owner: u32, members: &[Vec<u32>]) -> Vec<u8> {
    let mut records = Vec::new();
    for pages in members {
        let mut map = vec![0; 69];
        for &number in pages {
            map[5 + number as usize / 8] |= 1 << (number % 8);
        }
        records.push((0, map));
    }
    let mut page = raw::data_page(owner, &records);
    raw::put16(&mut page, 2, (PAGE - 14 - records.len() * 71) as u16);
    page
}

pub(super) fn append_record(
    after: &mut Vec<u8>,
    root: usize,
    bytes: &[u8],
    candidate: &mut u32,
    owned: &mut Vec<u32>,
    hints: &mut Vec<u32>,
) {
    let page = *candidate as usize * PAGE;
    let count = usize::from(u16::from_le_bytes(
        after[page + 12..page + 14].try_into().unwrap(),
    ));
    let end = if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(
                after[page + 14 + (count - 1) * 2..page + 16 + (count - 1) * 2]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        )
    };
    if bytes.len() + 2 > end - 14 - count * 2 {
        hints.retain(|number| number != candidate);
        *candidate = (after.len() / PAGE) as u32;
        owned.push(*candidate);
        hints.push(*candidate);
        after.extend(data_page(&[]));
    }
    let page = *candidate as usize * PAGE;
    let count = usize::from(u16::from_le_bytes(
        after[page + 12..page + 14].try_into().unwrap(),
    ));
    let end = if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(
                after[page + 14 + (count - 1) * 2..page + 16 + (count - 1) * 2]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        )
    };
    let start = end - bytes.len();
    after[page + start..page + end].copy_from_slice(bytes);
    raw::put16(after, page + 14 + count * 2, start as u16 | 0x8000);
    raw::put16(after, page + 12, (count + 1) as u16);
    raw::put16(after, page + 2, (start - 14 - (count + 1) * 2) as u16);
    let root_at = 12 * PAGE + 14 + root * 2;
    let root_start =
        usize::from(u16::from_le_bytes(after[root_at..root_at + 2].try_into().unwrap()) & 0x1fff);
    let original_length = if root == 0 {
        4096 - root_start
    } else {
        let previous = usize::from(
            u16::from_le_bytes(after[root_at - 2..root_at].try_into().unwrap()) & 0x1fff,
        );
        previous - root_start
    };
    after[12 * PAGE + root_start..12 * PAGE + root_start + original_length].fill(0);
    raw::put32(
        after,
        12 * PAGE + root_start,
        (*candidate << 8) | count as u32,
    );
    raw::put16(after, root_at, root_start as u16 | 0x4000);
}

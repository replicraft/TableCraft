//! Literal C2/V0 rows and two aliased integral physical indexes.
use super::{PAGE, base, raw};

fn map(members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &member in members {
        bytes[5 + member / 8] |= 1 << (member % 8);
    }
    bytes
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn data(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = raw::data_page(owner, rows);
    let used: usize = rows.iter().map(|(_, row)| row.len() + 2).sum();
    raw::put16(&mut page, 2, (PAGE - 14 - used) as u16);
    page
}

fn leaf(values: [i32; 3]) -> Vec<u8> {
    let mut page = vec![0x9b; PAGE];
    page[..480].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    raw::put32(&mut page, 4, 10);
    for (slot, value) in values.into_iter().enumerate() {
        let offset = 480 + slot * 9;
        page[offset] = 0x7f;
        page[offset + 1..offset + 5].copy_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
        page[offset + 5..offset + 9].copy_from_slice(&((12_u32 << 8) | slot as u32).to_be_bytes());
        let end = (slot + 1) * 9;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    raw::put16(&mut page, 2, (PAGE - 480 - 27) as u16);
    page
}

fn complete_empty_tail(bytes: &mut [u8], number: usize) {
    let page = &mut bytes[number * PAGE..(number + 1) * PAGE];
    let count = usize::from(u16::from_le_bytes(page[45..47].try_into().unwrap()));
    let mut tail = 63 + count * 25;
    for _ in 0..count {
        let length = usize::from(u16::from_le_bytes(page[tail..tail + 2].try_into().unwrap()));
        tail += 2 + length;
    }
    raw::put16(page, tail, 0xffff);
}

pub(super) fn source() -> (Vec<u8>, Vec<u8>, usize, usize) {
    let mut before = base::database();
    before.resize(17 * PAGE, 0xa7);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = 5;

    for (definition, map_page, members) in [(2, 3, vec![4]), (7, 8, vec![])] {
        complete_empty_tail(&mut before, definition);
        raw::put32(
            &mut before,
            definition * PAGE + 59,
            ((map_page as u32) << 8) | 1,
        );
        install(
            &mut before,
            map_page,
            &data(0, &[(0, map(&members)), (0, map(&members))]),
        );
    }

    let catalog_rows = usize::from(u16::from_le_bytes(
        before[4 * PAGE + 12..4 * PAGE + 14].try_into().unwrap(),
    ));
    let last = super::row_span(&before, 4, catalog_rows - 1);
    raw::put16(
        &mut before,
        4 * PAGE + 2,
        (last.start - 4 * PAGE - 14 - catalog_rows * 2) as u16,
    );

    let mut definition = base::definition(&[("Id", 4, 0, 4), ("Score", 4, 4, 4)], 3, 11 << 8);
    let names_end = 63 + 50 + 6 + 12;
    definition.copy_within(63..names_end, 87);
    definition[63..87].fill(0);
    raw::put32(&mut definition, 47, 4);
    raw::put32(&mut definition, 51, 2);
    raw::put32(&mut definition, 59, (11 << 8) | 1);
    for physical in 0..2 {
        raw::put32(&mut definition, 67 + physical * 12, 3);
    }
    let physical_start = names_end + 24;
    for physical in 0..2 {
        let at = physical_start + physical * 52;
        for column in 0..10 {
            raw::put16(&mut definition, at + 4 + column * 3, 0xffff);
        }
        raw::put16(&mut definition, at + 4, physical as u16);
        definition[at + 6] = 1;
        raw::put32(&mut definition, at + 34, (15 << 8) | physical as u32);
        raw::put32(&mut definition, at + 38, 13 + physical as u32);
        definition[at + 46] = 0x81;
    }
    let logical_start = physical_start + 104;
    let mut tail = logical_start + 112;
    for (logical, name) in ["IdKey", "IdAlias", "ScoreKey", "ScoreAlias"]
        .into_iter()
        .enumerate()
    {
        let at = logical_start + logical * 28;
        raw::put32(&mut definition, at + 4, logical as u32);
        raw::put32(&mut definition, at + 8, (logical / 2) as u32);
        let encoded: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        raw::put16(&mut definition, tail, encoded.len() as u16);
        tail += 2;
        definition[tail..tail + encoded.len()].copy_from_slice(&encoded);
        tail += encoded.len();
    }
    raw::put16(&mut definition, tail, 0xffff);
    install(&mut before, 10, &definition);
    install(
        &mut before,
        11,
        &data(0, &[(0, map(&[12])), (0, map(&[12]))]),
    );
    install(
        &mut before,
        15,
        &data(0, &[(0, map(&[13])), (0, map(&[14]))]),
    );
    let rows = [(1_i32, 10_i32), (2, 20), (3, 30)]
        .into_iter()
        .map(|(id, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    install(&mut before, 12, &data(10, &rows));
    install(&mut before, 13, &leaf([1, 2, 3]));
    install(&mut before, 14, &leaf([10, 20, 30]));

    let mut expected = before.clone();
    let first = super::row_span(&before, 12, 0);
    assert_eq!(first.len(), 11);
    assert_eq!(
        &before[first.start + 6..first.start + 10],
        &10_i32.to_le_bytes()
    );
    expected[first.start + 6..first.start + 10].copy_from_slice(&11_i32.to_le_bytes());
    install(&mut expected, 14, &leaf([11, 20, 30]));
    (before, expected, physical_start, 15)
}

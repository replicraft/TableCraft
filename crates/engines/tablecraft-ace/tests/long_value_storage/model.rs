//! Literal ordinary-column maps and all three long-value descriptor forms.
use super::{fixture, support};

pub const PAGE: usize = 4096;
pub const TAIL: usize = 164;

pub fn descriptor(tag: u32, length: u32, reference: u32) -> Vec<u8> {
    [
        (tag | length).to_le_bytes().as_slice(),
        &reference.to_le_bytes(),
        &[0; 4],
    ]
    .concat()
}

fn inline(value: &[u8]) -> Vec<u8> {
    let mut result = descriptor(0x8000_0000, value.len() as u32, 0);
    result.extend(value);
    result
}

fn row(id: i32, memo: Option<&[u8]>, blob: Option<&[u8]>) -> Vec<u8> {
    let mut result = vec![3, 0];
    result.extend(id.to_le_bytes());
    if let Some(memo) = memo {
        result.extend(memo);
    }
    let middle = result.len() as u16;
    if let Some(blob) = blob {
        result.extend(blob);
    }
    let end = result.len() as u16;
    result.extend(end.to_le_bytes());
    result.extend(middle.to_le_bytes());
    result.extend(6_u16.to_le_bytes());
    result.extend(2_u16.to_le_bytes());
    result.push(1 | if memo.is_some() { 2 } else { 0 } | if blob.is_some() { 4 } else { 0 });
    result
}

pub fn map(pages: &[usize]) -> Vec<u8> {
    let mut result = vec![0; 8];
    for page in pages {
        result[5 + page / 8] |= 1 << (page % 8);
    }
    result
}

pub fn page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut result = support::data_page(owner, rows);
    let count = rows.len();
    let start = if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(result[12 + 2 * count..14 + 2 * count].try_into().unwrap()) & 0x1fff,
        )
    };
    support::put16(&mut result, 2, (start - 14 - 2 * count) as u16);
    result
}

pub fn maps(bytes: &mut [u8], memo_owned: Vec<u8>) {
    let rows = vec![
        (0, map(&[12])),
        (0, memo_owned),
        (0, map(&[20])),
        (0, map(&[17, 18, 19])),
        (0, map(&[19])),
    ];
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&page(0, &rows));
}

pub fn database(code: u8) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize(21 * PAGE, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Blob", 11, 0, 0)],
        5,
        11 << 8,
    );
    definition[88] = 12;
    definition[103] = 2;
    definition[128] = 2;
    for (position, column, owned, hint) in [
        (TAIL, 1, (11 << 8) | 1, (11 << 8) | 2),
        (TAIL + 10, 2, (11 << 8) | 3, (11 << 8) | 4),
    ] {
        support::put16(&mut definition, position, column);
        support::put32(&mut definition, position + 2, owned);
        support::put32(&mut definition, position + 6, hint);
    }
    support::put16(&mut definition, TAIL + 20, 65535);
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    maps(&mut bytes, map(&[14, 15, 16, 20]));
    let empty = inline(&[]);
    let memo_inline = inline(&[65, 0]);
    let blob_inline = inline(&[0, 255]);
    let memo_single = descriptor(0x4000_0000, 2, 14 << 8);
    let blob_single = descriptor(0x4000_0000, 1, 17 << 8);
    let memo_chain = descriptor(0, 4, 15 << 8);
    let blob_chain = descriptor(0, 3, 18 << 8);
    let rows = vec![
        (0, row(1, None, None)),
        (0, row(2, Some(&empty), Some(&empty))),
        (0, row(3, Some(&memo_inline), Some(&blob_inline))),
        (0, row(4, Some(&memo_single), Some(&blob_single))),
        (0, row(5, Some(&memo_chain), Some(&blob_chain))),
    ];
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&page(10, &rows));
    for (number, payload) in [
        (14, vec![0x2d, 0x4e]),
        (
            15,
            [(16_u32 << 8).to_le_bytes().as_slice(), &[66, 0]].concat(),
        ),
        (16, [0_u32.to_le_bytes().as_slice(), &[67, 0]].concat()),
        (17, vec![0xa5]),
        (18, [(19_u32 << 8).to_le_bytes().as_slice(), &[1]].concat()),
        (19, [0_u32.to_le_bytes().as_slice(), &[2, 3]].concat()),
        (20, vec![0xff, 0, 0x12]),
    ] {
        let mut value = page(0, &[(0, payload)]);
        value[4..8].copy_from_slice(b"LVAL");
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&value);
    }
    let mut global = vec![0; 69];
    for number in 21..512 {
        global[5 + number / 8] |= 1 << (number % 8);
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&page(1, &[(0, global)]));
    bytes
}

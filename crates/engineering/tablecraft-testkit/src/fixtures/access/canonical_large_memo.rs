//! Independent ordinary native17 Memo records and an exact selected-row patch oracle.
use super::{canonical_creation, canonical_native17_creation, data_page, put16, put32};

pub struct Source {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub last_page: usize,
}

fn packed(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut page = data_page(owner, rows);
    let used = 14 + 2 * rows.len() + rows.iter().map(|row| row.1.len()).sum::<usize>();
    put16(&mut page, 2, (4096 - used) as u16);
    page
}

fn map(count: usize, members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; 5 + count.div_ceil(8)];
    for page in members {
        bytes[5 + page / 8] |= 1 << (page % 8);
    }
    bytes
}

pub fn source(count: usize, target: usize) -> Source {
    assert!(count > 0 && target < count);
    let mut before = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let pages = count.div_ceil(120);
    let end_page = 8 + pages;
    before.resize(end_page * 4096, 0);
    let definition = &mut before[7 * 4096..8 * 4096];
    put32(definition, 16, count as u32);
    put16(definition, 43, 1);
    definition[88] = 12;
    definition[99..103].copy_from_slice(&[9, 4, 0, 1]);
    definition[103] = 2;
    put16(definition, 109, 0);
    put16(definition, 111, 0);
    // Native17 catalog conversion retains 22 map rows; the user's new Memo
    // owned/candidate declarations select distinct empty rows 22 and 23.
    definition[131..143].copy_from_slice(&[1, 0, 22, 3, 0, 0, 23, 3, 0, 0, 255, 255]);
    put32(definition, 8, 143);
    put16(definition, 2, 3945);

    let mut maps = vec![
        (0, map(64, [4])),
        (0, map(64, [])),
        (0, map(64, [])),
        (0, map(end_page, 8..end_page)),
        (0, map(64, [4])),
        (0, map(64, [])),
        (0, map(64, [])),
        (0, map(end_page, 8..end_page)),
    ];
    maps.extend((0..16).map(|_| (0, map(64, []))));
    before[3 * 4096..4 * 4096].copy_from_slice(&packed(0, &maps));
    before[4096..8192].copy_from_slice(&packed(1, &[(0, map(end_page, []))]));
    for index in 0..pages {
        let rows = (index * 120..((index + 1) * 120).min(count))
            .map(|ordinal| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend((ordinal as i32 + 1).to_le_bytes());
                row.extend([6, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0]);
                row.extend([b'o', 0, b'l', 0, b'd', 0]);
                row.extend([24, 0, 6, 0, 1, 0, 3]);
                (0, row)
            })
            .collect::<Vec<_>>();
        let page = 8 + index;
        before[page * 4096..(page + 1) * 4096].copy_from_slice(&packed(7, &rows));
    }
    let last_page = end_page - 1;
    let target_page = 8 + target / 120;
    let directory = target_page * 4096 + 14 + 2 * (target % 120);
    let start = u16::from_le_bytes(before[directory..directory + 2].try_into().unwrap()) as usize;
    // NO_CLONE: Keep the immutable BEFORE oracle while independently patching
    // AFTER; copy-on-write storage would still need this complete payload copy.
    let mut after = before.clone();
    let text = target_page * 4096 + start + 18;
    after[text..text + 6].copy_from_slice(&[b'n', 0, b'e', 0, b'w', 0]);
    Source {
        before,
        after,
        last_page,
    }
}

use super::{fixture, support};

pub const PAGE_BYTES: usize = 4096;
pub const FIRST_DATA_PAGE: usize = 14;
const BITMAP_PAGE: usize = 13;
const USAGE_PAGE: usize = 11;
const TABLE_DEFINITION: usize = 10;

// Original synthetic one-row-per-page table; declared geometry and independent
// bitmap bytes deliberately do not import production layout constants.
pub fn database(pages: usize, indirect: bool, bad_tail: bool) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize((FIRST_DATA_PAGE + pages) * PAGE_BYTES, 0);
    support::put32(&mut bytes, TABLE_DEFINITION * PAGE_BYTES + 16, pages as u32);
    let bits = FIRST_DATA_PAGE + pages + usize::from(bad_tail);
    let mut bitmap = vec![0; bits.div_ceil(8)];
    for page in FIRST_DATA_PAGE..FIRST_DATA_PAGE + pages {
        bitmap[page / 8] |= 1 << (page % 8);
    }
    if bad_tail {
        bitmap[(bits - 1) / 8] |= 1 << ((bits - 1) % 8);
    }
    let map = if indirect {
        bytes[BITMAP_PAGE * PAGE_BYTES..(BITMAP_PAGE + 1) * PAGE_BYTES].fill(0);
        bytes[BITMAP_PAGE * PAGE_BYTES..BITMAP_PAGE * PAGE_BYTES + 4]
            .copy_from_slice(&[5, 1, 0, 0]);
        bytes[BITMAP_PAGE * PAGE_BYTES + 4..BITMAP_PAGE * PAGE_BYTES + 4 + bitmap.len()]
            .copy_from_slice(&bitmap);
        [vec![1], (BITMAP_PAGE as u32).to_le_bytes().to_vec()].concat()
    } else {
        [vec![0; 5], bitmap].concat()
    };
    bytes[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(1, &[(0, map)]));
    for index in 0..pages {
        let id = index as i32 + 1;
        let row = [
            2_u16.to_le_bytes().as_slice(),
            &id.to_le_bytes(),
            &(id * 10).to_le_bytes(),
            &[3],
        ]
        .concat();
        let page = FIRST_DATA_PAGE + index;
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(TABLE_DEFINITION as u32, &[(0, row)]));
    }
    bytes
}

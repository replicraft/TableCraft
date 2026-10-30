//! Literal relocation and complete indirect GLOBAL allocation, independent of edits.
use super::{Case, PAGE, raw, row_span};

const FIRST_HIGH_PAGE: usize = 16384;

fn replace_member(bytes: &mut [u8], map_page: usize, slot: usize, member: usize) {
    let span = row_span(bytes, map_page, slot);
    assert_eq!(span.len(), 69);
    let base = if member >= FIRST_HIGH_PAGE {
        FIRST_HIGH_PAGE
    } else {
        0
    };
    let row = &mut bytes[span];
    row.fill(0);
    row[1..5].copy_from_slice(&(base as u32).to_le_bytes());
    let relative = member - base;
    assert!(relative < 512);
    row[5 + relative / 8] |= 1 << (relative % 8);
}

fn relocate_leaf(bytes: &mut [u8], old_root: usize, new_root: usize, data: usize) {
    let mut page = bytes[old_root * PAGE..(old_root + 1) * PAGE].to_vec();
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &10_u32.to_le_bytes());
    assert_eq!(&page[24..26], &[0, 0]);
    for slot in 0..3 {
        let start = 480 + slot * 9;
        assert_eq!(page[start], 0x7f);
        assert_eq!(
            &page[start + 5..start + 9],
            &((12_u32 << 8) | slot as u32).to_be_bytes(),
        );
        page[start + 5..start + 9]
            .copy_from_slice(&(((data as u32) << 8) | slot as u32).to_be_bytes());
    }
    bytes[new_root * PAGE..(new_root + 1) * PAGE].copy_from_slice(&page);
}

fn global(bytes: &mut [u8], bitmap_page: usize) {
    // An indirect GLOBAL map explicitly represents the entire retained image.
    // A zero free bit means allocated, including every opaque tail page. No
    // absent bitmap cell is used to invent allocation for a selected high page.
    assert!(bytes.len() / PAGE <= (PAGE - 4) * 8);
    let mut row = vec![1];
    row.extend((bitmap_page as u32).to_le_bytes());
    let mut map = raw::data_page(1, &[(0, row)]);
    raw::put16(&mut map, 2, (PAGE - 14 - 2 - 5) as u16);
    bytes[PAGE..2 * PAGE].copy_from_slice(&map);

    let bitmap = &mut bytes[bitmap_page * PAGE..(bitmap_page + 1) * PAGE];
    bitmap.fill(0);
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
}

pub(super) fn apply(case: &mut Case) {
    let old_roots = if case.shape.preserved() {
        vec![27, 28, 29]
    } else {
        vec![13, 14]
    };
    let length = case.shape.length();
    for bytes in [&mut case.before, &mut case.expected] {
        let data = bytes[12 * PAGE..13 * PAGE].to_vec();
        let old_map = if case.shape.preserved() { 30 } else { 15 };
        let index_map = bytes[old_map * PAGE..(old_map + 1) * PAGE].to_vec();
        bytes.resize(length, 0xa7);
        bytes[case.index_map * PAGE..(case.index_map + 1) * PAGE].copy_from_slice(&index_map);
        bytes[case.data * PAGE..(case.data + 1) * PAGE].copy_from_slice(&data);
        replace_member(bytes, 11, 0, case.data);
        replace_member(bytes, 11, 1, case.data);

        for (physical, (&old_root, &new_root)) in old_roots.iter().zip(&case.roots).enumerate() {
            relocate_leaf(bytes, old_root, new_root, case.data);
            raw::put32(
                bytes,
                10 * PAGE + case.physical_start + physical * 52 + 38,
                new_root as u32,
            );
            raw::put32(
                bytes,
                10 * PAGE + case.physical_start + physical * 52 + 34,
                ((case.index_map as u32) << 8) | physical as u32,
            );
            replace_member(bytes, case.index_map, physical, new_root);
        }
        global(bytes, case.global_bitmap);
    }
}

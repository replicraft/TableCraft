use super::*;
use crate::format::allocation::tests::{budget, header, map_file};
use std::{cell::Cell, io::Cursor};

fn reader<'a>(
    source: &'a mut Cursor<Vec<u8>>,
    budget: &'a mut tablecraft_core::limits::ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    pages: u32,
) -> Reader<'a, Cursor<Vec<u8>>> {
    Reader {
        source,
        header: header(pages),
        budget,
        cancelled,
        reads: 0,
    }
}

#[test]
fn dense_membership_exceeds_list_limit_without_retaining_one_reference_per_page() {
    let mut bytes = map_file(true);
    bytes[2 * 4096 + 4..2 * 4096 + 4 + 1024].fill(255);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let before = allowance.remaining(tablecraft_core::limits::Resource::Bytes);
    let mut reader = reader(&mut source, &mut allowance, &|| false, 8192);
    let mut membership = PageMembership::new(&mut reader, 1 << 8).unwrap();
    for page in [8191, 0, 4096, 7, 8190, 8] {
        assert!(membership.contains(&mut reader, page).unwrap());
    }
    assert!(
        before
            - reader
                .budget
                .remaining(tablecraft_core::limits::Resource::Bytes)
            < 20 * 1024
    );
}

#[test]
fn inline_and_indirect_queries_match_independent_membership_in_arbitrary_order() {
    for indirect in [false, true] {
        let mut source = Cursor::new(map_file(indirect));
        let mut allowance = budget();
        let mut reader = reader(&mut source, &mut allowance, &|| false, 256);
        let mut membership = PageMembership::new(&mut reader, 1 << 8).unwrap();
        for page in (0..256).rev() {
            assert_eq!(
                membership.contains(&mut reader, page).unwrap(),
                [3, 7, 63, 64, 127].contains(&page)
            );
        }
    }
}

#[test]
fn late_bitmap_preflight_and_cache_reloads_retain_validation() {
    let mut bytes = map_file(true);
    let row = usize::from(u16::from_le_bytes(
        bytes[4096 + 14..4096 + 16].try_into().unwrap(),
    ));
    bytes[4096 + row + 5..4096 + row + 9].copy_from_slice(&3_u32.to_le_bytes());
    bytes[3 * 4096..3 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
    bytes[3 * 4096 + 4] = 1;
    let mut source = Cursor::new(bytes.clone());
    let mut allowance = budget();
    assert!(matches!(
        PageMembership::new(
            &mut reader(&mut source, &mut allowance, &|| false, 256),
            1 << 8
        ),
        Err(InspectError::Malformed)
    ));

    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = reader(&mut source, &mut allowance, &|| false, 65536);
    let mut membership = PageMembership::new(&mut reader, 1 << 8).unwrap();
    assert!(membership.contains(&mut reader, 3).unwrap());
    reader.source.get_mut()[3 * 4096] = 255;
    assert_eq!(
        membership.contains(&mut reader, 32736),
        Err(InspectError::Malformed)
    );
}

#[test]
fn cancellation_is_checked_for_each_membership_lookup() {
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let mut source = Cursor::new(map_file(false));
    let mut allowance = budget();
    let mut reader = reader(&mut source, &mut allowance, &callback, 256);
    let mut membership = PageMembership::new(&mut reader, 1 << 8).unwrap();
    cancelled.set(true);
    assert_eq!(
        membership.contains(&mut reader, 3),
        Err(InspectError::Cancelled)
    );
}

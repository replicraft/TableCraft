use super::*;
use crate::format::allocation::tests::{budget, header, map_file};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

fn reader<'a>(
    source: &'a mut Cursor<Vec<u8>>,
    allowance: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    pages: u32,
) -> Reader<'a, Cursor<Vec<u8>>> {
    Reader {
        source,
        header: header(pages),
        budget: allowance,
        cancelled,
        reads: 0,
    }
}
#[test]
fn inline_and_indirect_iteration_preserve_sorted_pages_and_repeatable_exhaustion() {
    for indirect in [false, true] {
        let mut source = Cursor::new(map_file(indirect));
        let mut allowance = budget();
        let mut reader = reader(&mut source, &mut allowance, &|| false, 256);
        let mut cursor = PageCursor::new(&mut reader, 1 << 8).unwrap();
        let mut pages = Vec::new();
        while let Some(page) = cursor.next(&mut reader).unwrap() {
            pages.push(page);
        }
        assert_eq!(pages, [3, 7, 63, 64, 127]);
        assert_eq!(cursor.next(&mut reader).unwrap(), None);
        assert_eq!(cursor.next(&mut reader).unwrap(), None);
    }
}
#[test]
fn dense_maps_exceed_the_materialized_list_limit_without_per_address_allocation() {
    let mut bytes = map_file(true);
    bytes[2 * 4096 + 4..2 * 4096 + 4 + 1024].fill(255);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let start = allowance.remaining(tablecraft_core::limits::Resource::Bytes);
    let mut reader = reader(&mut source, &mut allowance, &|| false, 8192);
    let mut cursor = PageCursor::new(&mut reader, 1 << 8).unwrap();
    for expected in 0..8192 {
        assert_eq!(cursor.next(&mut reader).unwrap(), Some(expected));
    }
    assert_eq!(cursor.next(&mut reader).unwrap(), None);
    assert!(
        start
            - reader
                .budget
                .remaining(tablecraft_core::limits::Resource::Bytes)
            < 16 * 1024
    );
}
#[test]
fn all_metadata_is_validated_before_first_page_and_reread_bitmaps_are_rechecked() {
    let mut bytes = map_file(true);
    // The later indirect slot must be validated before exposing first-bucket pages.
    let row = usize::from(u16::from_le_bytes(
        bytes[4096 + 14..4096 + 16].try_into().unwrap(),
    ));
    bytes[4096 + row + 5..4096 + row + 9].copy_from_slice(&3_u32.to_le_bytes());
    bytes[3 * 4096..3 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
    bytes[3 * 4096 + 4] = 1;
    let mut source = Cursor::new(bytes.clone());
    let mut allowance = budget();
    assert!(matches!(
        PageCursor::new(
            &mut reader(&mut source, &mut allowance, &|| false, 256),
            1 << 8
        ),
        Err(InspectError::Malformed)
    ));
    // Give that bucket a valid logical range, then corrupt it after validation.
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = reader(&mut source, &mut allowance, &|| false, 65536);
    let mut cursor = PageCursor::new(&mut reader, 1 << 8).unwrap();
    for expected in [3, 7, 63, 64, 127] {
        assert_eq!(cursor.next(&mut reader).unwrap(), Some(expected));
    }
    reader.source.get_mut()[3 * 4096] = 255;
    assert_eq!(cursor.next(&mut reader), Err(InspectError::Malformed));
}
#[test]
fn constructor_and_iteration_keep_cancellation_and_work_limits() {
    let mut source = Cursor::new(map_file(false));
    let mut allowance = budget();
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let mut reader = reader(&mut source, &mut allowance, &cancel, 256);
    let mut cursor = PageCursor::new(&mut reader, 1 << 8).unwrap();
    cancelled.set(true);
    assert_eq!(cursor.next(&mut reader), Err(InspectError::Cancelled));
    let mut source = Cursor::new(map_file(false));
    let mut allowance = ResourceBudget::new(0, 0, 1000);
    assert!(matches!(
        PageCursor::new(
            &mut self::reader(&mut source, &mut allowance, &|| false, 256),
            1 << 8
        ),
        Err(InspectError::Limit(_))
    ));
}
proptest! {
    #![proptest_config(proptest::test_runner::Config { cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4d4150435552534f),..Default::default() })]
    #[test]
    fn arbitrary_bitmaps_match_independent_scalar_positions(bits in prop::collection::vec(any::<u8>(),0..=28),indirect in any::<bool>()) {
        let mut file=map_file(indirect);
        if indirect { file[2*4096+4..3*4096].fill(0);file[2*4096+4..2*4096+4+bits.len()].copy_from_slice(&bits); }
        else {
            let mut map=vec![0;5];map.extend_from_slice(&bits);
            let page=4096;let start=4096-map.len();file[page+14..page+16].copy_from_slice(&(start as u16).to_le_bytes());file[page+start..page+4096].copy_from_slice(&map);
        }
        let mut source=Cursor::new(file);let mut allowance=budget();let mut reader=reader(&mut source,&mut allowance,&||false,256);
        let mut cursor=PageCursor::new(&mut reader,1<<8).unwrap();let mut actual=Vec::new();while let Some(page)=cursor.next(&mut reader).unwrap(){actual.push(page);}
        let expected=(0..bits.len()*8).filter(|bit|bits[bit/8]&(1<<(bit%8))!=0).map(|bit|bit as u32).collect::<Vec<_>>();
        prop_assert_eq!(actual,expected);
    }
}

#[test]
fn malformed_roots_and_empty_extreme_inline_bases_preserve_checked_behavior() {
    for (map, expected) in [
        (vec![9], InspectError::Unsupported),
        (vec![0, 0, 0], InspectError::Truncated),
        (vec![1, 0], InspectError::Malformed),
        (vec![1, 0, 1, 0, 0], InspectError::Malformed),
    ] {
        let mut file = map_file(false);
        let start = 4096 - map.len();
        file[4096 + 14..4096 + 16].copy_from_slice(&(start as u16).to_le_bytes());
        file[4096 + start..8192].copy_from_slice(&map);
        let mut source = Cursor::new(file);
        let mut allowance = budget();
        assert!(
            matches!(PageCursor::new(&mut reader(&mut source,&mut allowance,&||false,256),1<<8),Err(error) if error==expected)
        );
    }
    let mut file = map_file(false);
    let map = [vec![0], u32::MAX.to_le_bytes().to_vec(), vec![0; 16]].concat();
    let start = 4096 - map.len();
    file[4096 + 14..4096 + 16].copy_from_slice(&(start as u16).to_le_bytes());
    file[4096 + start..8192].copy_from_slice(&map);
    let mut source = Cursor::new(file);
    let mut allowance = budget();
    let mut reader = reader(&mut source, &mut allowance, &|| false, 256);
    let mut cursor = PageCursor::new(&mut reader, 1 << 8).unwrap();
    assert_eq!(cursor.next(&mut reader).unwrap(), None);
}

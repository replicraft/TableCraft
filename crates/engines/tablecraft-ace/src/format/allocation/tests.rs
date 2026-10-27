use super::*;
use crate::Header;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

pub(crate) fn header(pages: u32) -> Header {
    let mut bytes = [0; 66];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    Header::parse(&bytes, u64::from(pages) * 4096).unwrap()
}
fn decode_bitmap(
    bytes: &[u8],
    base: u32,
    page_count: u32,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, InspectError> {
    let mut pages = Vec::new();
    let mut empty = Cursor::new([]);
    bitmap(
        &mut Reader {
            source: &mut empty,
            header: header(page_count),
            budget: allowance,
            cancelled,
            reads: 0,
        },
        bytes,
        base,
        &mut pages,
    )?;
    Ok(pages)
}
pub(super) fn budget() -> ResourceBudget {
    ResourceBudget::new(4 * 1024 * 1024, 0, 1_000_000)
}

#[test]
fn byte_word_and_partial_block_boundaries_produce_strictly_ordered_unique_pages() {
    let mut bytes = [0; 17];
    for bit in [0, 7, 63, 64, 127, 128, 135] {
        bytes[bit / 8] |= 1 << (bit % 8);
    }
    assert_eq!(
        decode_bitmap(&bytes, 32, 256, &mut budget(), &|| false).unwrap(),
        [32, 39, 95, 96, 159, 160, 167]
    );
    assert!(
        decode_bitmap(&[0; 16], 0, 256, &mut budget(), &|| false)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        decode_bitmap(&[0x80], 250, 256, &mut budget(), &|| false),
        Err(InspectError::Malformed)
    ));
    assert!(matches!(
        decode_bitmap(&[0, 1], u32::MAX - 1, 256, &mut budget(), &|| false),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn page_count_and_caller_limits_remain_bounded_and_cancellable() {
    assert!(matches!(
        decode_bitmap(&[0xff; 513], 0, 8192, &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    ));
    for mut allowance in [
        ResourceBudget::new(0, 0, 100),
        ResourceBudget::new(1000, 0, 7),
    ] {
        assert!(matches!(
            decode_bitmap(&[0xff; 8], 0, 256, &mut allowance, &|| false),
            Err(InspectError::Limit(_))
        ));
    }
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 1
    };
    assert!(matches!(
        decode_bitmap(&[0xff; 16], 0, 256, &mut budget(), &cancelled),
        Err(InspectError::Cancelled)
    ));
    assert_eq!(checks.get(), 2);
}

pub(super) fn map_file(indirect: bool) -> Vec<u8> {
    let mut file = vec![0; 256 * 4096];
    let mut map = if indirect {
        vec![1, 2, 0, 0, 0, 0, 0, 0, 0]
    } else {
        vec![0, 0, 0, 0, 0]
    };
    let bits = [0x88, 0, 0, 0, 0, 0, 0, 0x80, 1, 0, 0, 0, 0, 0, 0, 0x80];
    if indirect {
        file[2 * 4096..2 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
        file[2 * 4096 + 4..2 * 4096 + 4 + bits.len()].copy_from_slice(&bits);
    } else {
        map.extend(bits);
    }
    file[4096..4098].copy_from_slice(&[1, 1]);
    file[4096 + 12..4096 + 14].copy_from_slice(&1_u16.to_le_bytes());
    let start = 4096 - map.len();
    file[4096 + 14..4096 + 16].copy_from_slice(&(start as u16).to_le_bytes());
    file[4096 + start..8192].copy_from_slice(&map);
    file
}
fn decode_map(file: Vec<u8>) -> Result<Vec<u32>, InspectError> {
    let mut cursor = Cursor::new(file);
    let mut allowance = budget();
    pages(
        &mut Reader {
            source: &mut cursor,
            header: header(256),
            budget: &mut allowance,
            cancelled: &|| false,
            reads: 0,
        },
        1 << 8,
    )
}
#[test]
fn inline_and_indirect_maps_keep_the_same_sorted_page_contract() {
    for indirect in [false, true] {
        assert_eq!(decode_map(map_file(indirect)).unwrap(), [3, 7, 63, 64, 127]);
    }
    let mut file = map_file(true);
    file[2 * 4096] = 0;
    assert!(matches!(decode_map(file), Err(InspectError::Malformed)));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x5441_424c_554d_4150),..Default::default()})]
    #[test]
    fn emitted_pages_match_an_independent_bit_model(bytes in prop::collection::vec(any::<u8>(), 0..128), base in 0_u32..1000) {
        let expected: Vec<_> = bytes.iter().enumerate().flat_map(|(byte, value)| {
            (0..8).filter(move |bit| value & (1 << bit) != 0).map(move |bit| base + (byte * 8 + bit) as u32)
        }).collect();
        let actual = decode_bitmap(&bytes, base, 2048, &mut budget(), &|| false).unwrap();
        prop_assert_eq!(&actual, &expected);
        prop_assert!(actual.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

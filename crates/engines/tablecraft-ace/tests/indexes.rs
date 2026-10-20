use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_table_indexes};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
#[path = "indexes/ranges.rs"]
mod ranges;
#[path = "indexes/text.rs"]
mod text;
fn copy() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
#[test]
fn native_index_metadata_keeps_logical_physical_and_key_order() {
    let bytes = copy();
    let mut source = Cursor::new(bytes.clone());
    let result = inspect_table_indexes(
        &mut source,
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        result
            .indexes()
            .iter()
            .map(|x| (
                x.name(),
                x.number(),
                x.data_number(),
                x.root_page(),
                x.native_type(),
                x.flags()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("COLOR_ID", 0, 0, 729, 0, 0x80),
            ("Number", 1, 1, 730, 0, 0x81),
            ("ORDR_NBR", 2, 2, 731, 0, 0x81)
        ]
    );
    assert_eq!(
        result
            .indexes()
            .iter()
            .map(|x| (x.columns()[0].number(), x.columns()[0].descending()))
            .collect::<Vec<_>>(),
        vec![(2, false), (1, false), (0, false)]
    );
    assert_eq!(source.into_inner(), bytes);
}
#[test]
fn invalid_index_definitions_limits_and_cancellation_return_no_partial_metadata() {
    for change in 0..7 {
        let mut bytes = copy();
        let base = 726 * 4096;
        match change {
            0 => put32(&mut bytes, base + 394, 3),
            1 => put16(&mut bytes, base + 234, 500),
            2 => put32(&mut bytes, base + 268, 827),
            3 => put16(&mut bytes, base + 470, 5000),
            4 => bytes[base + 236] = 2,
            5 => put32(&mut bytes, base + 418, 0),
            _ => put32(&mut bytes, base + 47, u32::MAX),
        }
        assert!(
            inspect_table_indexes(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| false
            )
            .is_err(),
            "{change}"
        );
    }
    assert_eq!(
        inspect_table_indexes(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(
        inspect_table_indexes(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut ResourceBudget::new(66, 1, 100),
            &|| false
        )
        .is_err()
    );
}
proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x494e444558),..Default::default()})]
    #[test]
    fn mutated_index_metadata_is_bounded(edits in prop::collection::vec((230_usize..600,any::<u8>()),1..20)) {
        let mut bytes=copy();for (at,value) in edits {bytes[726*4096+at]=value;}
        let _=inspect_table_indexes(&mut Cursor::new(bytes),CatalogScope::new(1),"ROULETTE_TB",&mut budget(),&||false);
    }
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn indexed_reads_follow_native_keys_and_fetch_exact_live_rows() {
    let bytes = copy();
    let ordered = tablecraft_ace::read_table_index(
        &mut Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ordr_nbr",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        ordered
            .rows()
            .iter()
            .map(|row| row[0].clone())
            .collect::<Vec<_>>(),
        (1..=38)
            .map(tablecraft_core::values::Value::Int32)
            .collect::<Vec<_>>()
    );
    let colors = tablecraft_ace::read_table_index(
        &mut Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "COLOR_ID",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(colors.rows().len(), 38);
    let mut a = ordered.rows().to_vec();
    let mut b = colors.rows().to_vec();
    let id = |row: &Vec<tablecraft_core::values::Value>| match row[0] {
        tablecraft_core::values::Value::Int32(n) => n,
        _ => panic!("id carrier"),
    };
    a.sort_by_key(id);
    b.sort_by_key(id);
    assert_eq!(a, b);
    // Native index corruption is observable even when sequential records remain valid.
    let mut broken = bytes;
    broken[731 * 4096] = 0;
    assert!(
        tablecraft_ace::read_table(
            &mut Cursor::new(broken.clone()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    assert_eq!(indexed_error(broken), Some(InspectError::Malformed));
}
fn indexed_error(bytes: Vec<u8>) -> Option<InspectError> {
    tablecraft_ace::read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ORDR_NBR",
        &mut budget(),
        &|| false,
    )
    .err()
}
#[test]
fn invalid_index_pages_keys_references_links_and_cycles_are_rejected() {
    for change in 0..9 {
        let mut bytes = copy();
        let base = 731 * 4096;
        match change {
            0 => put32(&mut bytes, base + 4, 2),
            1 => put16(&mut bytes, base + 24, 14),
            2 => bytes[base + 27] |= 1,
            3 => put16(&mut bytes, base + 2, 0),
            4 => put32(&mut bytes, base + 16, 731),
            5 => bytes[base + 480] = 0x80,
            6 => bytes[base + 484] = 0,
            7 => bytes[base + 487] = 2,
            _ => bytes[728 * 4096 + 21] |= 0x80,
        }
        assert!(indexed_error(bytes).is_some(), "{change}");
    }
    let mut bytes = copy();
    let base = 731 * 4096;
    bytes[base..base + 4096].fill(0);
    bytes[base..base + 2].copy_from_slice(&[3, 1]);
    put16(&mut bytes, base + 2, 3616);
    put32(&mut bytes, base + 4, 726);
    put32(&mut bytes, base + 20, 731);
    assert_eq!(indexed_error(bytes), Some(InspectError::Malformed));
}
#[test]
fn indexed_read_profiles_limits_and_cancellation_never_fall_back() {
    assert_eq!(
        tablecraft_ace::read_table_index(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "MSysObjects",
            "ParentIdName",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::Unsupported)
    );
    assert_eq!(
        tablecraft_ace::read_table_index(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            "private-missing-index",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::UnknownIndex)
    );
    assert_eq!(
        tablecraft_ace::read_table_index(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(
        tablecraft_ace::read_table_index(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            "ORDR_NBR",
            &mut ResourceBudget::new(16 * 1024 * 1024, 146, 1_000_000),
            &|| false
        )
        .is_err()
    );
}
proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x494e4452454144),..Default::default()})]
    #[test]
    fn mutated_index_pages_return_without_panics(edits in prop::collection::vec((0_usize..4096,any::<u8>()),1..20)) {
        let mut bytes=copy();for (at,value) in edits {bytes[731*4096+at]=value;}
        let _=indexed_error(bytes);
    }
}

fn index_page(leaf: bool, entries: &[Vec<u8>], previous: u32, next: u32, tail: u32) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
    put32(&mut page, 4, 726);
    put32(&mut page, 12, previous);
    put32(&mut page, 16, next);
    put32(&mut page, 20, tail);
    let mut end = 0;
    for entry in entries {
        page[480 + end..480 + end + entry.len()].copy_from_slice(entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    put16(&mut page, 2, (3616 - end) as u16);
    page
}
fn two_level_tree() -> Vec<u8> {
    let mut bytes = copy();
    let entries = bytes[731 * 4096 + 480..731 * 4096 + 480 + 38 * 9]
        .as_chunks::<9>()
        .0
        .iter()
        .map(|x| x.to_vec())
        .collect::<Vec<_>>();
    let mut separator = entries[18].clone();
    separator.extend(730_u32.to_be_bytes());
    bytes[729 * 4096..730 * 4096].copy_from_slice(&index_page(false, &[separator], 0, 0, 731));
    bytes[730 * 4096..731 * 4096].copy_from_slice(&index_page(true, &entries[..19], 0, 731, 0));
    bytes[731 * 4096..732 * 4096].copy_from_slice(&index_page(true, &entries[19..], 730, 0, 0));
    put32(&mut bytes, 726 * 4096 + 372, 729); // Physical index 2's root.
    bytes[727 * 4096 + 3756] = 0x0e; // Owned pages 729, 730, 731.
    bytes
}
#[test]
fn interior_children_leaf_links_and_separator_bounds_are_checked() {
    let bytes = two_level_tree();
    let result = tablecraft_ace::read_table_index(
        &mut Cursor::new(bytes.clone()),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ORDR_NBR",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 38);
    for change in 0..3 {
        let mut bytes = bytes.clone();
        match change {
            0 => put32(&mut bytes, 730 * 4096 + 16, 0),
            1 => bytes[729 * 4096 + 484] = 18,
            _ => put32(&mut bytes, 729 * 4096 + 20, 730),
        }
        assert_eq!(indexed_error(bytes), Some(InspectError::Malformed));
    }
}
#[test]
fn tree_depth_is_structurally_bounded() {
    let mut bytes = two_level_tree();
    bytes.resize(843 * 4096, 0);
    bytes[729 * 4096..730 * 4096].copy_from_slice(&index_page(false, &[], 0, 0, 827));
    for page in 827..843 {
        bytes[page * 4096..(page + 1) * 4096].copy_from_slice(&index_page(
            false,
            &[],
            0,
            0,
            (page + 1) as u32,
        ));
        let bit = page - 728;
        bytes[727 * 4096 + 3756 + bit / 8] |= 1 << (bit % 8);
    }
    assert_eq!(indexed_error(bytes), Some(InspectError::StructureLimit));
}

#[test]
fn indexed_live_roots_follow_bounded_relocation_to_a_deleted_target_slot() {
    let mut bytes = copy();
    let root = 728 * 4096;
    let start = (u16::from_le_bytes([bytes[root + 20], bytes[root + 21]]) & 0x1fff) as usize;
    let end = (u16::from_le_bytes([bytes[root + 18], bytes[root + 19]]) & 0x1fff) as usize;
    let record = bytes[root + start..root + end].to_vec();
    bytes.resize(828 * 4096, 0);
    let target = 827 * 4096;
    bytes[target..target + 2].copy_from_slice(&[1, 1]);
    put32(&mut bytes, target + 4, 726);
    put16(&mut bytes, target + 12, 1);
    put16(
        &mut bytes,
        target + 14,
        ((4096 - record.len()) as u16) | 0x8000,
    );
    bytes[target + 4096 - record.len()..target + 4096].copy_from_slice(&record);
    put16(&mut bytes, root + 20, (start as u16) | 0x4000);
    put32(&mut bytes, root + start, 827 << 8);
    let rows = tablecraft_ace::read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        "ORDR_NBR",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), 38);
    assert_eq!(rows.rows()[0][0], tablecraft_core::values::Value::Int32(1));
}

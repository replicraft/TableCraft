use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{InspectError, inspect_table_indexes};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
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

#[path = "support/page_reads.rs"]
mod page_reads;
mod support;
use proptest::prelude::*;
use std::io::Cursor;
use support::*;
use tablecraft_ace::{InspectError, inspect_catalog};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};
fn inspect(bytes: Vec<u8>) -> Result<tablecraft_ace::CatalogInspection, InspectError> {
    inspect_catalog(
        &mut Cursor::new(bytes),
        CatalogScope::new(9),
        &mut ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
}

#[test]
fn catalog_decodes_both_allocation_maps_and_relocated_rows() {
    for overflow in [false, true] {
        for indirect in [false, true] {
            let bytes = image(overflow, indirect);
            let original = bytes.clone();
            let mut source = Cursor::new(bytes);
            let result = inspect_catalog(
                &mut source,
                CatalogScope::new(9),
                &mut ResourceBudget::new(16 * 1024 * 1024, 100, 10_000),
                &|| false,
            )
            .unwrap();
            assert_eq!(source.into_inner(), original);
            assert_eq!(result.catalog().objects().len(), 2);
            let form = result
                .catalog()
                .lookup("form", Some(ObjectKind::Form), None)
                .unwrap()
                .unwrap();
            assert_eq!(form.parent(), Some(CatalogScope::new(9).object_id(100)));
            assert_eq!(
                result
                    .catalog()
                    .get(form.parent().unwrap())
                    .unwrap()
                    .name()
                    .as_str(),
                "Table"
            );
            assert_eq!(result.origins()[1].type_code(), -32768);
            assert_eq!(result.origins()[1].native_id(), -1);
        }
    }
}
#[test]
fn catalog_reuses_each_scanned_page_and_still_reads_relocation_targets() {
    for relocated in [false, true] {
        let mut source = page_reads::PageReads::new(image(relocated, false));
        let result = inspect_catalog(
            &mut source,
            CatalogScope::new(9),
            &mut ResourceBudget::new(1_000_000, 100, 10_000),
            &|| false,
        )
        .unwrap();
        assert_eq!(result.catalog().objects().len(), 2);
        assert_eq!(source.count(4), 1, "one read for both live catalog roots");
        assert_eq!(source.count(5), if relocated { 2 } else { 0 });
    }
}
#[test]
fn malformed_pointers_offsets_counts_nulls_and_cycles_fail_without_partial_results() {
    let mut b = image(true, false);
    let p = 4 * 4096;
    let start = usize::from(u16::from_le_bytes([b[p + 16], b[p + 17]]) & 0x1fff);
    put32(&mut b, p + start, (4 << 8) | 1);
    assert!(inspect(b).is_err());
    let mut b = image(false, false);
    put32(&mut b, 2 * 4096 + 4, 3);
    assert_eq!(inspect(b).err(), Some(InspectError::Malformed));
    let mut b = image(false, false);
    put16(&mut b, 4 * 4096 + 14, 1);
    assert!(inspect(b).is_err());
    let mut b = image(false, false);
    put32(&mut b, 2 * 4096 + 16, 3);
    assert!(inspect(b).is_err());
    let mut b = image(false, false);
    b[4 * 4096 + 4095] &= !(1 << 2);
    assert!(inspect(b).is_err());
    let mut b = image(false, false);
    put16(&mut b, 4 * 4096 + 4096 - 5, 0);
    assert!(inspect(b).is_err());
}
#[test]
fn unsupported_and_encoded_formats_reject_catalog_decoding() {
    let mut b = image(false, false);
    b[20] = 4;
    assert_eq!(inspect(b).err(), Some(InspectError::Unsupported));
    let mut b = image(false, false);
    b[62] ^= 1;
    assert_eq!(inspect(b).err(), Some(InspectError::Encoded));
    let mut b = image(false, false);
    b[21] = 1;
    assert!(inspect(b).is_ok());
    let mut b = image(false, false);
    b[21] = 2;
    assert_eq!(inspect(b).err(), Some(InspectError::Unsupported));
}
#[test]
fn independent_limits_and_cancellation_stop_catalog_work() {
    let bytes = image(false, false);
    for mut budget in [
        ResourceBudget::new(66, 100, 10_000),
        ResourceBudget::new(1_000_000, 0, 10_000),
        ResourceBudget::new(1_000_000, 100, 1),
    ] {
        assert!(
            inspect_catalog(
                &mut Cursor::new(bytes.clone()),
                CatalogScope::new(1),
                &mut budget,
                &|| false
            )
            .is_err()
        );
    }
    assert_eq!(
        inspect_catalog(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut ResourceBudget::new(1_000_000, 100, 10_000),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
}
#[test]
fn original_candidate_copy_decodes_metadata_only_with_unknown_types_retained() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(path).unwrap();
    let copy = original.clone();
    let mut source = Cursor::new(copy);
    let result = inspect_catalog(
        &mut source,
        CatalogScope::new(123),
        &mut ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.into_inner(), original);
    assert_eq!(result.catalog().objects().len(), 146);
    let objects = result.catalog();
    for (kind, count) in [
        (ObjectKind::Table, 36),
        (ObjectKind::Query, 59),
        (ObjectKind::Form, 13),
        (ObjectKind::Report, 8),
        (ObjectKind::Macro, 3),
        (ObjectKind::Module, 12),
        (ObjectKind::Container, 9),
        (ObjectKind::Relationship, 1),
        (ObjectKind::Unknown, 5),
    ] {
        assert_eq!(
            objects
                .objects()
                .iter()
                .filter(|o| o.kind() == kind)
                .count(),
            count
        );
    }
    assert!(
        objects
            .lookup("USysRibbons", Some(ObjectKind::Table), None)
            .unwrap()
            .is_some()
    );
    assert!(
        objects
            .lookup("AutoKeys", Some(ObjectKind::Macro), None)
            .unwrap()
            .is_some()
    );
    assert!(result.origins().iter().any(|o| o.type_code() == -32757));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x434154414c4f4701),
        ..Default::default()
    })]
    #[test]
    fn mutated_catalog_structures_return_without_panics(
        edits in prop::collection::vec((0_usize..7*4096, any::<u8>()), 1..24),
        overflow in any::<bool>(),
        indirect in any::<bool>(),
    ) {
        let mut bytes = image(overflow, indirect);
        for (offset, value) in edits { bytes[offset] = value; }
        let _ = inspect(bytes);
    }
}

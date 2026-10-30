use tablecraft_testkit::access::mutation_fixture as base;
#[path = "guid_indexes/fixture.rs"]
mod fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection,
    Int32Update, Int64IndexSelection, NativeImage, WriteBlocker, check_table, read_table_index,
    read_table_index_range, read_table_int64_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8]) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
}
fn pairs(table: &tablecraft_ace::TableSnapshot) -> Vec<(Option<[u8; 16]>, i32)> {
    table
        .rows()
        .iter()
        .map(|row| {
            let Value::Int32(id) = row[0] else {
                panic!("ID")
            };
            let key = match row[1] {
                Value::Guid(value) => Some(value.to_bytes()),
                Value::Null => None,
                _ => panic!("GUID carrier"),
            };
            (key, id)
        })
        .collect()
}

#[test]
fn guid_segments_preserve_canonical_bits_nulls_zero_and_compressed_row_references() {
    let values = [
        Some([0; 16]),
        Some([0xff; 16]),
        Some([0x80; 16]),
        None,
        None,
        Some([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
    ];
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = fixture::database(code, descending, &values, 0);
            let mut expected = values
                .iter()
                .copied()
                .enumerate()
                .map(|(i, key)| (key, i as i32 + 1))
                .collect::<Vec<_>>();
            expected.sort_by(|a, b| {
                let cmp = a.0.cmp(&b.0);
                (if descending { cmp.reverse() } else { cmp }).then(a.1.cmp(&b.1))
            });
            assert_eq!(pairs(&read(&bytes).unwrap()), expected);
            assert_eq!(
                check_table(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .indexes()[0]
                    .status(),
                IndexCheckStatus::Checked
            );
            let repeated = fixture::database(code, descending, &[Some([0x80; 16]); 3], 22);
            assert_eq!(
                pairs(&read(&repeated).unwrap()),
                vec![
                    (Some([0x80; 16]), 1),
                    (Some([0x80; 16]), 2),
                    (Some([0x80; 16]), 3)
                ]
            );
            assert_eq!(
                pairs(&read(&fixture::database(code, descending, &[None; 3], 4)).unwrap()),
                vec![(None, 1), (None, 2), (None, 3)]
            );
        }
    }
}

#[test]
fn guid_index_constraints_distinguish_null_zero_and_all_128_bits() {
    let mut next = [0; 16];
    next[15] = 1;
    for (values, flags, expected) in [
        (vec![Some([0; 16]), Some(next), None, None], 0x81, None),
        (
            vec![Some([0; 16]), Some([0; 16])],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![None, Some([0; 16])],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (vec![Some([0; 16])], 0x88, None),
    ] {
        let mut bytes = fixture::database(6, false, &values, 0);
        numeric_fixture::index_flags(&mut bytes, flags);
        assert_eq!(
            check_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            expected.map(InspectError::IndexConstraint)
        );
    }
}

#[test]
fn guid_tags_key_agreement_integer_seeks_and_incomplete_composites_are_strict() {
    let bytes = fixture::database(6, true, &[Some([0x80; 16])], 0);
    for change in 0..7 {
        let mut changed = bytes.clone();
        match change {
            0 => changed[14 * 4096 + 489] = 0xf6, // Intermediate 09 must not be complemented.
            1 => changed[14 * 4096 + 498] = 8,    // Final tag must be complemented descending.
            2 => changed[14 * 4096 + 497] ^= 1,   // Canonical key disagrees with stored field.
            3 => changed[14 * 4096 + 480] = 0x7f, // Wrong direction marker.
            4 => {
                support::put16(&mut changed, 10 * 4096 + 143 + 7, 0);
                changed[10 * 4096 + 143 + 9] = 1;
            } // Composite metadata is missing its second encoded component.
            5 => support::put16(&mut changed, 10 * 4096 + 75 + 25 + 23, 15),
            _ => support::put16(&mut changed, 14 * 4096 + 24, 28),
        }
        assert!(read(&changed).is_err());
    }
    for selection in [Int32IndexSelection::All, Int32IndexSelection::Null] {
        assert_eq!(
            read_table_index_range(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                selection,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
    for selection in [Int64IndexSelection::All, Int64IndexSelection::Null] {
        assert_eq!(
            read_table_int64_index_range(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                selection,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
    for (mut allowance, cancel) in [
        (budget(), true),
        (ResourceBudget::new(66, 100, 10_000), false),
        (ResourceBudget::new(1_000_000, 0, 10_000), false),
        (ResourceBudget::new(1_000_000, 100, 0), false),
    ] {
        assert!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut allowance,
                &|| cancel
            )
            .is_err()
        );
    }
    assert!(
        read(&fixture::database(6, false, &[], 0))
            .unwrap()
            .rows()
            .is_empty()
    );
}

#[test]
fn read_coverage_does_not_grant_ace_or_guid_writes_and_preserves_unrelated_jet4_changes() {
    let mut bytes = fixture::database(6, false, &[Some([0; 16]), Some([0xff; 16])], 0);
    let capture = |bytes: &[u8]| {
        NativeImage::capture(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
    let plan = capture(&bytes)
        .plan_int32_updates(
            "Items",
            "Id",
            &[Int32Update::new(0, 1, 11)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        &bytes[14 * 4096..15 * 4096],
        &plan.after_bytes()[14 * 4096..15 * 4096]
    );
    let row = u16::from_le_bytes([bytes[12 * 4096 + 14], bytes[12 * 4096 + 15]]) as usize;
    let field = 12 * 4096 + row + 2..12 * 4096 + row + 6;
    for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
        assert!(before == after || field.contains(&offset));
    }
    assert_eq!(
        pairs(&read(plan.after_bytes()).unwrap()),
        vec![(Some([0; 16]), 11), (Some([0xff; 16]), 2)]
    );
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x47554944494e4458),..Default::default()})]
    #[test]
    fn arbitrary_guid_keys_match_independent_canonical_byte_order(values in prop::collection::vec(prop::option::of(any::<[u8;16]>()),0..40),descending in any::<bool>()) {
        let bytes=fixture::database(6,descending,&values,0);
        let mut expected=values.iter().copied().enumerate().map(|(i,key)|(key,i as i32+1)).collect::<Vec<_>>();
        expected.sort_by(|a,b|{let cmp=a.0.cmp(&b.0);(if descending{cmp.reverse()}else{cmp}).then(a.1.cmp(&b.1))});
        prop_assert_eq!(pairs(&read(&bytes).unwrap()),expected);
    }
}

#[path = "guid_indexes/selections.rs"]
mod selections;

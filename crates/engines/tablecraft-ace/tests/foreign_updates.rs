#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod base;
#[path = "foreign_updates/boolean.rs"]
mod boolean;
#[path = "foreign_updates/fixture.rs"]
mod fixture;
mod support;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    ConstraintViolation, EditError, FixedScalarUpdate, IndexCheckStatus, IndexConstraintViolation,
    InspectError, Int32Update, NativeImage, WriteBlocker, check_table, check_table_constraints,
    read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn preview(
    bytes: &[u8],
    field: &str,
    row: usize,
    old: Value,
    new: Value,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_fixed_updates(
        "Items",
        field,
        &[FixedScalarUpdate::new(row, old, new)],
        &mut budget(),
        &|| false,
    )
}
fn verify(bytes: &[u8]) {
    assert!(
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .iter()
        .all(|i| i.status() == IndexCheckStatus::Checked)
    );
    assert!(
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
    for index in ["Primary", "ParentLink", "ChildLink", "ChildAlias"] {
        assert_eq!(
            read_table_index(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                index,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows()
            .len(),
            3
        );
    }
}

#[test]
fn checked_reciprocal_and_ordinary_aliases_reparent_clear_and_restore_without_other_changes() {
    for code in [5, 6] {
        for descending in [false, true] {
            for cascade in [false, true] {
                let bytes = fixture::database(code, descending, cascade);
                verify(&bytes);
                let changed = preview(&bytes, "Score", 0, Value::Null, Value::Int32(2)).unwrap();
                verify(changed.candidate_bytes());
                assert_eq!(changed.changed_rows(), 1);
                assert_eq!(
                    &changed.candidate_bytes()[..12 * PAGE_BYTES],
                    &bytes[..12 * PAGE_BYTES]
                );
                assert_eq!(
                    &changed.candidate_bytes()[13 * PAGE_BYTES..16 * PAGE_BYTES],
                    &bytes[13 * PAGE_BYTES..16 * PAGE_BYTES]
                );
                let rows = read_table(
                    &mut Cursor::new(changed.candidate_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    rows.rows(),
                    &[
                        vec![Value::Int32(1), Value::Int32(2)],
                        vec![Value::Int32(2), Value::Int32(1)],
                        vec![Value::Int32(3), Value::Int32(1)]
                    ]
                );
                let cleared = preview(
                    changed.candidate_bytes(),
                    "Score",
                    0,
                    Value::Int32(2),
                    Value::Null,
                )
                .unwrap();
                verify(cleared.candidate_bytes());
                let noops = preview(&bytes, "Score", 0, Value::Null, Value::Null).unwrap();
                assert_eq!(noops.candidate_bytes(), bytes);
                // An unreferenced parent key can move without modifying child values.
                let parent = preview(&bytes, "Id", 2, Value::Int32(3), Value::Int32(4)).unwrap();
                verify(parent.candidate_bytes());
                assert_eq!(
                    &parent.candidate_bytes()[14 * PAGE_BYTES..],
                    &bytes[14 * PAGE_BYTES..]
                );
            }
        }
    }
}

#[test]
fn orphans_required_unique_and_missing_cascades_reject_the_complete_candidate() {
    for cascade in [false, true] {
        let bytes = fixture::database(6, false, cascade);
        for (field, row, old, new, expected) in [
            (
                "Score",
                0,
                Value::Null,
                Value::Int32(99),
                EditError::Inspect(InspectError::Constraint(ConstraintViolation::MissingParent)),
            ),
            (
                "Id",
                0,
                Value::Int32(1),
                Value::Int32(4),
                EditError::Inspect(InspectError::Constraint(ConstraintViolation::MissingParent)),
            ),
            (
                "Id",
                0,
                Value::Int32(1),
                Value::Int32(2),
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey,
                )),
            ),
            (
                "Id",
                0,
                Value::Int32(1),
                Value::Null,
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::NullKey,
                )),
            ),
            (
                "Score",
                1,
                Value::Int32(2),
                Value::Null,
                EditError::Conflict,
            ),
        ] {
            assert_eq!(preview(&bytes, field, row, old, new).err(), Some(expected));
        }
        verify(&bytes);
    }
}

#[test]
fn malformed_links_and_jet4_foreign_tables_cannot_gain_a_write_plan() {
    let original = fixture::database(6, false, false);
    for (offset, value) in [(259 + 28 + 12, 3), (259 + 28 + 17, 0), (259 + 28 + 21, 1)] {
        let mut bytes = original.clone();
        bytes[10 * PAGE_BYTES + offset] = value;
        assert!(preview(&bytes, "Score", 0, Value::Null, Value::Int32(2)).is_err());
    }
    let mut hidden = original.clone();
    for logical in [1, 2] {
        hidden[10 * PAGE_BYTES + 259 + logical * 28 + 23] = 0;
    }
    // Complete ordinary physical reads alone cannot qualify a disguised graph.
    assert!(
        check_table(
            &mut Cursor::new(&hidden),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .iter()
        .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert!(preview(&hidden, "Score", 0, Value::Null, Value::Int32(2)).is_err());
    let mut jet = original;
    jet[4..20].copy_from_slice(b"Standard Jet DB\0");
    jet[20] = 1;
    assert_eq!(
        capture(&jet)
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(2, 3, 4)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}

#[test]
fn late_cancellation_and_resource_exhaustion_publish_no_foreign_candidate() {
    let bytes = fixture::database(6, false, false);
    let calls = Cell::new(0);
    let run = |stop| {
        capture(&bytes).preview_ace16_fixed_updates(
            "Items",
            "Score",
            &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(2))],
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        )
    };
    run(usize::MAX).unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(run(stop).is_err());
    }
    assert!(
        capture(&bytes)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(2))],
                &mut ResourceBudget::new(0, 100_000, 10_000_000),
                &|| false
            )
            .is_err()
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x464f524549474e),..Default::default()})]
    #[test]
    fn nullable_foreign_updates_match_a_parent_membership_model(key in prop::option::of(0i32..6),descending in any::<bool>()) {
        let bytes=fixture::database(6,descending,false);
        let result=preview(&bytes,"Score",0,Value::Null,key.map_or(Value::Null,Value::Int32));
        if key.is_none_or(|key|(1..=3).contains(&key)) {
            let candidate=result.unwrap();verify(candidate.candidate_bytes());
            let rows=read_table(&mut Cursor::new(candidate.candidate_bytes()),CatalogScope::new(1),"Items",&mut budget(),&||false).unwrap();
            prop_assert_eq!(&rows.rows()[0][1],&key.map_or(Value::Null,Value::Int32));
            prop_assert_eq!(candidate.changed_rows(),usize::from(key.is_some()));
        }
        else {prop_assert!(result.is_err());}
    }
}

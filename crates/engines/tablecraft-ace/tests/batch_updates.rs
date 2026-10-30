#![cfg(feature = "ace16-write-preview")]
use tablecraft_testkit::access::mutation_fixture as base;
#[path = "foreign_updates/fixture.rs"]
mod foreign_fixture;
#[path = "mask_updates/fixture.rs"]
mod mask_fixture;
#[path = "batch_updates/multiple_tables.rs"]
mod multiple_tables;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
#[path = "batch_updates/rules.rs"]
mod rules;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, check_table,
    check_table_constraints, read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
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
fn run(
    bytes: &[u8],
    groups: &[FixedColumnUpdates<'_>],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_batch_updates(groups, &mut budget(), &|| false)
}
fn rows(bytes: &[u8], table: &str) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}
fn checks(bytes: &[u8], table: &str) {
    assert!(
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            table,
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .iter()
        .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
}

#[test]
fn complete_composite_keys_validate_once_even_when_either_intermediate_state_is_duplicate() {
    for code in [5, 6] {
        let mut bytes = numeric_fixture::database(
            code,
            &[4, 4],
            &[(0, false), (1, true)],
            &[
                vec![Some(1), Some(10)],
                vec![Some(1), Some(20)],
                vec![Some(2), Some(10)],
            ],
            0,
        );
        numeric_fixture::index_flags(&mut bytes, 0x81);
        let a = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(2))];
        let b = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(20),
        )];
        for (column, updates) in [("Id", a.as_slice()), ("Score", b.as_slice())] {
            assert!(
                capture(&bytes)
                    .preview_ace16_fixed_updates("Items", column, updates, &mut budget(), &|| false)
                    .is_err()
            );
        }
        let candidate = run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Id", &a),
                FixedColumnUpdates::new("Items", "Score", &b),
            ],
        )
        .unwrap();
        assert_eq!(candidate.changed_rows(), 1);
        assert_eq!(
            rows(candidate.candidate_bytes(), "Items"),
            vec![
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(1), Value::Int32(20)],
                vec![Value::Int32(2), Value::Int32(10)]
            ]
        );
        checks(candidate.candidate_bytes(), "Items");
        let reversed = run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Score", &b),
                FixedColumnUpdates::new("Items", "Id", &a),
            ],
        )
        .unwrap();
        assert_eq!(candidate.candidate_bytes(), reversed.candidate_bytes());
    }
}

#[test]
fn multiple_scalar_and_boolean_fields_merge_only_their_mask_bits_and_count_one_row() {
    let bytes = mask_fixture::database(6, &[true]);
    let guid = Guid::from_bytes([0x33; 16]);
    let input = [
        ("First", Value::Boolean(true), Value::Boolean(false)),
        ("Tiny", Value::Null, Value::Byte(0)),
        ("Number", Value::Null, Value::Int32(-5)),
        ("Large", Value::Null, Value::Int64(i64::MIN)),
        ("Guid", Value::Null, Value::Guid(guid)),
        ("Other", Value::Byte(31), Value::Byte(32)),
        ("Guard", Value::Boolean(false), Value::Boolean(true)),
        ("Seventh", Value::Boolean(true), Value::Boolean(false)),
        ("Eighth", Value::Boolean(false), Value::Boolean(true)),
        ("Last", Value::Boolean(true), Value::Boolean(false)),
    ];
    let updates = input
        .iter()
        .map(|(_, old, new)| [FixedScalarUpdate::new(0, old.clone(), new.clone())])
        .collect::<Vec<_>>();
    let groups = input
        .iter()
        .zip(&updates)
        .map(|((name, _, _), updates)| FixedColumnUpdates::new("Items", name, updates))
        .collect::<Vec<_>>();
    let candidate = run(&bytes, &groups).unwrap();
    assert_eq!(candidate.changed_rows(), 1);
    assert_eq!(
        rows(candidate.candidate_bytes(), "Items"),
        vec![
            input
                .iter()
                .map(|(_, _, new)| new.clone())
                .collect::<Vec<_>>()
        ]
    );
    let row = 12 * 4096
        + (u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) as usize
            & 0x1fff);
    assert_eq!(&candidate.candidate_bytes()[row + 32..row + 34], &[0x7e, 1]);
    assert_eq!(&bytes[..row], &candidate.candidate_bytes()[..row]);
    assert_eq!(&bytes[row + 34..], &candidate.candidate_bytes()[row + 34..]);
    let reverse = groups.into_iter().rev().collect::<Vec<_>>();
    assert_eq!(
        run(&bytes, &reverse).unwrap().candidate_bytes(),
        candidate.candidate_bytes()
    );
}

#[test]
fn explicit_parent_and_child_changes_validate_the_complete_self_relationship() {
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = foreign_fixture::database(code, descending, false);
            let parent = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(4))];
            let children = [
                FixedScalarUpdate::new(1, Value::Int32(1), Value::Int32(4)),
                FixedScalarUpdate::new(2, Value::Int32(1), Value::Int32(4)),
            ];
            for groups in [
                vec![FixedColumnUpdates::new("Items", "Id", &parent)],
                vec![FixedColumnUpdates::new("Items", "Score", &children)],
            ] {
                assert!(run(&bytes, &groups).is_err());
            }
            let candidate = run(
                &bytes,
                &[
                    FixedColumnUpdates::new("Items", "Id", &parent),
                    FixedColumnUpdates::new("Items", "Score", &children),
                ],
            )
            .unwrap();
            assert_eq!(candidate.changed_rows(), 3);
            checks(candidate.candidate_bytes(), "Items");
            assert_eq!(
                rows(candidate.candidate_bytes(), "Items"),
                vec![
                    vec![Value::Int32(4), Value::Null],
                    vec![Value::Int32(2), Value::Int32(4)],
                    vec![Value::Int32(3), Value::Int32(4)]
                ]
            );
        }
    }
}

#[test]
fn duplicate_groups_stale_late_groups_and_duplicate_rows_reject_without_a_candidate() {
    let bytes = mask_fixture::database(6, &[true]);
    let valid = [FixedScalarUpdate::new(0, Value::Null, Value::Int32(10))];
    let stale = [FixedScalarUpdate::new(0, Value::Int64(9), Value::Int64(8))];
    assert_eq!(
        run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Number", &valid),
                FixedColumnUpdates::new("items", "NUMBER", &[])
            ]
        )
        .err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Number", &valid),
                FixedColumnUpdates::new("Items", "Large", &stale)
            ]
        )
        .err(),
        Some(EditError::Conflict)
    );
    let duplicated = [
        FixedScalarUpdate::new(0, Value::Null, Value::Byte(1)),
        FixedScalarUpdate::new(0, Value::Null, Value::Byte(2)),
    ];
    assert_eq!(
        run(
            &bytes,
            &[FixedColumnUpdates::new("Items", "Tiny", &duplicated)]
        )
        .err(),
        Some(EditError::Conflict)
    );
    let noop = run(&bytes, &[]).unwrap();
    assert_eq!(noop.changed_rows(), 0);
    assert_eq!(noop.candidate_bytes(), bytes);
    let noops = [FixedScalarUpdate::new(
        0,
        Value::Boolean(true),
        Value::Boolean(true),
    )];
    assert_eq!(
        run(&bytes, &[FixedColumnUpdates::new("Items", "First", &noops)])
            .unwrap()
            .candidate_bytes(),
        bytes
    );
}

#[test]
fn early_and_late_cancellation_and_cumulative_limits_publish_no_batch() {
    let bytes = mask_fixture::database(6, &[true]);
    let calls = Cell::new(0);
    let a = [FixedScalarUpdate::new(0, Value::Null, Value::Byte(1))];
    let b = [FixedScalarUpdate::new(0, Value::Null, Value::Int64(9))];
    let groups = [
        FixedColumnUpdates::new("Items", "Tiny", &a),
        FixedColumnUpdates::new("Items", "Large", &b),
    ];
    let probe = |stop| {
        capture(&bytes).preview_ace16_batch_updates(&groups, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() == stop
        })
    };
    probe(usize::MAX).unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(probe(stop).is_err());
    }
    for mut limit in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&bytes)
                .preview_ace16_batch_updates(&groups, &mut limit, &|| false)
                .is_err()
        );
    }
}

#[test]
fn structural_batch_limits_and_format_isolation_apply_before_field_staging() {
    use tablecraft_ace::{InspectError, WriteBlocker};
    let bytes = mask_fixture::database(6, &[true]);
    let too_many = (0..=NativeImage::MAX_FIXED_BATCH_GROUPS)
        .map(|_| FixedColumnUpdates::new("Items", "First", &[]))
        .collect::<Vec<_>>();
    assert_eq!(
        run(&bytes, &too_many).err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    let updates = (0..NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
        .map(|_| FixedScalarUpdate::new(0, Value::Null, Value::Null))
        .collect::<Vec<_>>();
    assert_eq!(
        run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Number", &updates),
                FixedColumnUpdates::new("Items", "Large", &updates)
            ]
        )
        .err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
    let mut jet = bytes;
    jet[4..20].copy_from_slice(b"Standard Jet DB\0");
    jet[20] = 1;
    assert_eq!(
        run(&jet, &[]).err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x42415443484b4559),..Default::default()})]
    #[test]
    fn both_tuple_components_change_against_the_original_model(a in prop::option::of(any::<i32>()),b in prop::option::of(any::<i64>()),reverse in any::<bool>()) {
        let bytes=numeric_fixture::database(6,&[4,0x13],&[(0,reverse),(1,!reverse)],&[vec![Some(1),Some(10)],vec![Some(2),Some(20)]],0);
        let first=[FixedScalarUpdate::new(0,Value::Int32(1),a.map_or(Value::Null,Value::Int32))];let second=[FixedScalarUpdate::new(0,Value::Int64(10),b.map_or(Value::Null,Value::Int64))];
        let candidate=run(&bytes,&[FixedColumnUpdates::new("Items","Id",&first),FixedColumnUpdates::new("Items","Score",&second)]).unwrap();
        prop_assert_eq!(&rows(candidate.candidate_bytes(),"Items")[0],&vec![a.map_or(Value::Null,Value::Int32),b.map_or(Value::Null,Value::Int64)]);
        checks(candidate.candidate_bytes(),"Items");
    }
}

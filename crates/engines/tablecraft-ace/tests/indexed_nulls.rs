#![cfg(feature = "ace16-write-preview")]
#[path = "indexed_nulls/capacity.rs"]
mod capacity;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use tablecraft_testkit::access as support;
#[path = "mutation/tree_fixture.rs"]
mod tree_fixture;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexConstraintViolation, InspectError, NativeImage,
    WriteBlocker, check_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const ENTRY_START: usize = 480;
const DEFINITION: usize = 10;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn database(code: u8, descending: bool, unique: bool, nullable: bool, topology: usize) -> Vec<u8> {
    let metadata = index_fixture::database(descending, true, nullable);
    let mut bytes = if topology == 0 {
        metadata.clone()
    } else if topology == 1 && !nullable && !descending {
        index_fixture::node_database()
    } else if topology >= 3 {
        tree_fixture::bounded_database(descending, nullable, topology == 4)
    } else {
        tree_fixture::database(descending, nullable, topology == 2)
    };
    bytes[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES]
        .copy_from_slice(&metadata[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[DEFINITION * PAGE_BYTES + 189] = if unique { 0x81 } else { 0x80 };
    for logical in 0..2 {
        bytes[DEFINITION * PAGE_BYTES + 195 + logical * 28 + 23] = 0;
    }
    bytes
}
fn preview(
    bytes: &[u8],
    updates: &[FixedScalarUpdate],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .preview_ace16_fixed_updates("Items", "Score", updates, &mut budget(), &|| false)
}
fn verify(before: &[u8], after: &[u8], expected: &[Option<i32>], descending: bool) {
    let mut model = expected
        .iter()
        .enumerate()
        .map(|(row, key)| (*key, row as i32 + 1))
        .collect::<Vec<_>>();
    model.sort_by(|a, b| {
        let key = a.0.cmp(&b.0);
        (if descending { key.reverse() } else { key }).then(a.1.cmp(&b.1))
    });
    for index in ["ScoreKey", "ScoreAlias"] {
        let table = read_table_index(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            index,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let actual = table
            .rows()
            .iter()
            .map(|row| {
                let key = match row[1] {
                    Value::Null => None,
                    Value::Int32(v) => Some(v),
                    _ => panic!("key"),
                };
                let Value::Int32(id) = row[0] else {
                    panic!("id")
                };
                (key, id)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, model);
    }
    assert!(
        check_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .iter()
        .all(|i| i.status() == tablecraft_ace::IndexCheckStatus::Checked)
    );
    let mut allowed = vec![false; before.len()];
    for (slot, value) in expected.iter().enumerate() {
        let at = 12 * PAGE_BYTES + 14 + 2 * slot;
        let row = 12 * PAGE_BYTES
            + (u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) as usize & 0x1fff);
        allowed[row + 6..row + 10].fill(true);
        allowed[row + 10] = true;
        assert_eq!((before[row + 10] ^ after[row + 10]) & !2, 0);
        if value.is_none() {
            assert_eq!(&before[row + 6..row + 10], &after[row + 6..row + 10]);
        }
    }
    for (number, page) in before.as_chunks::<PAGE_BYTES>().0.iter().enumerate() {
        if matches!(page[0], 3 | 4)
            && u32::from_le_bytes(page[4..8].try_into().unwrap()) == DEFINITION as u32
        {
            let start = number * PAGE_BYTES;
            let old_end = PAGE_BYTES - u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
            let new_end = PAGE_BYTES
                - u16::from_le_bytes(after[start + 2..start + 4].try_into().unwrap()) as usize;
            allowed[start + 2..start + 4].fill(true);
            allowed[start + 24..start + 26].fill(true);
            allowed[start + 27..start + old_end.max(new_end)].fill(true);
            assert_eq!(
                &before[start + old_end.max(new_end)..start + PAGE_BYTES],
                &after[start + old_end.max(new_end)..start + PAGE_BYTES]
            );
            assert!(new_end >= ENTRY_START);
        }
    }
    assert_eq!(before.len(), after.len());
    assert!(
        before
            .iter()
            .zip(after)
            .zip(allowed)
            .all(|((&a, &b), allowed)| a == b || allowed)
    );
}

#[test]
fn indexed_presence_changes_repack_leaf_node_deep_aliases_and_both_directions() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for topology in 0..5 {
                    let bytes = database(code, descending, unique, false, topology);
                    let candidate = preview(
                        &bytes,
                        &[FixedScalarUpdate::new(0, Value::Int32(10), Value::Null)],
                    )
                    .unwrap();
                    assert_eq!(candidate.changed_rows(), 1);
                    assert_eq!(candidate.before_bytes(), bytes);
                    verify(
                        &bytes,
                        candidate.candidate_bytes(),
                        &[None, Some(20), Some(30)],
                        descending,
                    );
                    let restored = preview(
                        candidate.candidate_bytes(),
                        &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(10))],
                    )
                    .unwrap();
                    verify(
                        candidate.candidate_bytes(),
                        restored.candidate_bytes(),
                        &[Some(10), Some(20), Some(30)],
                        descending,
                    );
                }
            }
        }
    }
}

#[test]
fn bounded_final_children_rewrite_all_separators_and_retain_topology() {
    for code in [5, 6] {
        for descending in [false, true] {
            for deep in [false, true] {
                for nullable in [false, true] {
                    let mut bytes = tree_fixture::bounded_database(descending, nullable, deep);
                    let metadata = database(code, descending, true, nullable, 0);
                    bytes[..PAGE_BYTES].copy_from_slice(&metadata[..PAGE_BYTES]);
                    bytes[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES].copy_from_slice(
                        &metadata[DEFINITION * PAGE_BYTES..(DEFINITION + 1) * PAGE_BYTES],
                    );
                    // Move the largest key to the start, requiring changes at
                    // every explicit bound, including the final root entry.
                    let candidate = preview(
                        &bytes,
                        &[FixedScalarUpdate::new(
                            2,
                            Value::Int32(30),
                            Value::Int32(-5),
                        )],
                    )
                    .unwrap();
                    let first = if nullable { None } else { Some(10) };
                    verify(
                        &bytes,
                        candidate.candidate_bytes(),
                        &[first, Some(20), Some(-5)],
                        descending,
                    );
                    let changed = preview(
                        candidate.candidate_bytes(),
                        &[FixedScalarUpdate::new(2, Value::Int32(-5), Value::Null)],
                    )
                    .unwrap();
                    verify(
                        candidate.candidate_bytes(),
                        changed.candidate_bytes(),
                        &[first, Some(20), None],
                        descending,
                    );
                    for number in if deep { vec![13, 17, 18] } else { vec![13] } {
                        let offset = number * PAGE_BYTES;
                        assert_eq!(
                            &changed.candidate_bytes()[offset + 20..offset + 24],
                            &[0; 4]
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nullable_expectations_ignore_stale_payload_and_noops_keep_exact_images() {
    let mut bytes = database(6, false, false, true, 0);
    let row = 12 * PAGE_BYTES
        + (u16::from_le_bytes(
            bytes[12 * PAGE_BYTES + 14..12 * PAGE_BYTES + 16]
                .try_into()
                .unwrap(),
        ) as usize
            & 0x1fff);
    support::put32(&mut bytes, row + 6, 12345);
    let candidate = preview(
        &bytes,
        &[FixedScalarUpdate::new(0, Value::Null, Value::Null)],
    )
    .unwrap();
    assert_eq!(candidate.changed_rows(), 0);
    assert_eq!(candidate.candidate_bytes(), bytes);
    let candidate = preview(
        &bytes,
        &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(20))],
    )
    .unwrap();
    verify(
        &bytes,
        candidate.candidate_bytes(),
        &[Some(20), Some(20), Some(30)],
        false,
    );
    assert_eq!(
        preview(
            &bytes,
            &[FixedScalarUpdate::new(0, Value::Int32(12345), Value::Null)]
        )
        .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn unique_required_and_field_rules_reject_the_whole_repacked_candidate() {
    let mut ignored = database(6, false, false, false, 0);
    ignored[DEFINITION * PAGE_BYTES + 189] = 0x82;
    assert_eq!(
        preview(
            &ignored,
            &[FixedScalarUpdate::new(0, Value::Int32(10), Value::Null)]
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
    let bytes = database(6, false, true, true, 1);
    assert_eq!(
        preview(
            &bytes,
            &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(20))]
        )
        .err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    );
    let mut bytes = database(6, false, true, false, 1);
    bytes[DEFINITION * PAGE_BYTES + 189] = 0x89;
    assert_eq!(
        preview(
            &bytes,
            &[FixedScalarUpdate::new(0, Value::Int32(10), Value::Null)]
        )
        .err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey
        )))
    );
    let mut bytes = index_fixture::with_rule(false, true, false, "ValidationRule", "Is Not Null");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    for logical in 0..2 {
        bytes[DEFINITION * PAGE_BYTES + 195 + logical * 28 + 23] = 0;
    }
    assert_eq!(
        preview(
            &bytes,
            &[FixedScalarUpdate::new(0, Value::Int32(10), Value::Null)]
        )
        .err(),
        Some(EditError::Inspect(InspectError::Constraint(
            tablecraft_ace::ConstraintViolation::FieldRule
        )))
    );
}

#[test]
fn late_cancellation_and_budgets_publish_no_nullable_candidate() {
    for topology in [2, 4] {
        let bytes = database(6, false, false, false, topology);
        let calls = Cell::new(0);
        let input = [FixedScalarUpdate::new(1, Value::Int32(20), Value::Null)];
        let image = || {
            NativeImage::capture(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        };
        image()
            .preview_ace16_fixed_updates("Items", "Score", &input, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                false
            })
            .unwrap();
        let total = calls.get();
        calls.set(0);
        assert_eq!(
            image()
                .preview_ace16_fixed_updates("Items", "Score", &input, &mut budget(), &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == total
                })
                .err(),
            Some(EditError::Inspect(InspectError::Cancelled))
        );
        assert!(
            image()
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &input,
                    &mut ResourceBudget::new(0, 100_000, 10_000_000),
                    &|| false
                )
                .is_err()
        );
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4e554c4c4b4559),..Default::default()})]
    #[test]
    fn nullable_batches_match_an_independent_key_row_model(a in prop::option::of(any::<i32>()),b in prop::option::of(any::<i32>()),descending in any::<bool>(),topology in 1usize..5) {
        let bytes=database(6,descending,false,true,topology);
        let value=|v:Option<i32>|v.map_or(Value::Null,Value::Int32);
        let candidate=preview(&bytes,&[FixedScalarUpdate::new(0,Value::Null,value(a)),FixedScalarUpdate::new(1,Value::Int32(20),value(b))]).unwrap();
        prop_assert_eq!(candidate.changed_rows(),usize::from(a.is_some())+usize::from(b!=Some(20)));
        verify(&bytes,candidate.candidate_bytes(),&[a,b,Some(30)],descending);
    }
}

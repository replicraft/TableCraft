//! Independent literal updates preserve counters, aliases and every INDEX byte.
use super::proof;
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan, NativeImage, WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_indexed::{self as sources, Source};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(119)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn replacement(source: &Source) -> Value {
    match source.rows[0][source.target_position] {
        Value::Int32(_) => Value::Int32(17),
        Value::Boolean(true) => Value::Boolean(false),
        _ => panic!("the independent target must be Score or originally true Flag"),
    }
}

fn literal_after(source: &Source) -> Vec<u8> {
    let mut expected = source.bytes.clone();
    let base = 8 * 4096;
    let start = usize::from(u16::from_le_bytes(
        expected[base + 14..base + 16].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    match source.rows[0][source.target_position] {
        Value::Int32(before) => {
            assert_eq!(source.target, "Score");
            assert_eq!(
                &expected[base + start + 6..base + start + 10],
                &before.to_le_bytes()
            );
            expected[base + start + 6..base + start + 10].copy_from_slice(&17_i32.to_le_bytes());
        }
        Value::Boolean(true) => {
            assert_eq!(source.target, "Flag");
            assert_eq!(&expected[base + start..base + start + 2], &[5, 0]);
            assert_eq!(expected[9 * 4096 - 1] & 0x10, 0x10);
            expected[9 * 4096 - 1] &= !0x10;
        }
        _ => panic!("literal expected image has no other target encoding"),
    }
    expected
}

#[test]
fn ordinary_counter_index_updates_match_eighteen_complete_literal_images() {
    let mut images = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            let mut source = sources::source(identity);
            if !source.ordinary {
                continue;
            }
            source.bytes[20] = header;
            let expected = literal_after(&source);
            let requests = [FixedScalarUpdate::new(
                0,
                source.rows[0][source.target_position].clone(),
                replacement(&source),
            )];
            let groups = [FixedColumnUpdates::new(
                source.table,
                source.target,
                &requests,
            )];
            let plan = image(&source.bytes)
                .plan_ordinary_fixed_updates(source.table, &groups, &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{}: {error:?}", identity.id()));
            assert_eq!(plan.before_bytes(), source.bytes);
            assert_eq!(plan.after_bytes(), expected, "{}", identity.id());
            assert_eq!(plan.changed_rows(), 1);
            assert_eq!(
                &plan.after_bytes()[7 * 4096..8 * 4096],
                &source.bytes[7 * 4096..8 * 4096]
            );
            for &number in &source.index_pages {
                let start = number as usize * 4096;
                assert_eq!(
                    &plan.after_bytes()[start..start + 4096],
                    &source.bytes[start..start + 4096]
                );
            }
            images += 1;
        }
    }
    assert_eq!(images, 18);
}

#[test]
fn readable_generated_or_indexed_keys_do_not_become_empty_write_targets() {
    for identity in sources::CASES {
        let source = sources::source(identity);
        if !source.ordinary {
            continue;
        }
        let groups = [FixedColumnUpdates::new(source.table, "Id", &[])];
        let result = image(&source.bytes).plan_ordinary_fixed_updates(
            source.table,
            &groups,
            &mut budget(),
            &|| false,
        );
        let expected = if source.last_counter.is_some() {
            WriteBlocker::Column
        } else {
            WriteBlocker::IndexedColumn
        };
        assert!(matches!(
            result,
            Err(EditError::Unsupported(blocker)) if blocker == expected
        ));
    }
}

#[test]
fn independent_ordinary_proof_binds_last_counter_and_inactive_index_tail() {
    for header in [5, 6] {
        let mut source = sources::source(sources::Case::InactiveIndexTail);
        source.bytes[20] = header;
        let requests = [FixedScalarUpdate::new(
            0,
            source.rows[0][source.target_position].clone(),
            replacement(&source),
        )];
        let groups = [FixedColumnUpdates::new(
            source.table,
            source.target,
            &requests,
        )];
        let mut observation = NativeEditPlan {
            before: source.bytes.clone(),
            after: literal_after(&source),
            changed_rows: 1,
        };
        proof::prove(
            &observation,
            scope(),
            source.table,
            &groups,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for at in [7 * 4096 + 20, 9 * 4096 + 498] {
            observation.after[at] ^= 1;
            assert!(matches!(
                proof::prove(
                    &observation,
                    scope(),
                    source.table,
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            observation.after[at] ^= 1;
        }
    }
}

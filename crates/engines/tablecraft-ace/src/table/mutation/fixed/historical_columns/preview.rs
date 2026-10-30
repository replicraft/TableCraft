//! APIs consume only the already qualified literal before/expected images.
use super::{budget, model, qualify};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, Int32Update, NativeImage,
    RowInsert, WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(nullable: bool) -> Vec<FixedScalarUpdate> {
    vec![
        FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(17)),
        FixedScalarUpdate::new(
            1,
            if nullable {
                Value::Null
            } else {
                Value::Int32(20)
            },
            Value::Int32(25),
        ),
    ]
}

fn older_source(variable: bool) -> model::Case {
    let selected_number = if variable { 4 } else { 2 };
    let mut case = model::case(5, false, variable, true, selected_number);
    let slot = 12 * model::PAGE + 16;
    let start = usize::from(u16::from_le_bytes(
        case.before[slot..slot + 2].try_into().unwrap(),
    ));
    // Row one already has a NULL Score and its original key is represented as
    // NULL. Its shorter historical count excludes Score without changing any
    // typed visible value; the complete raw old record is retained.
    case.before[12 * model::PAGE + start..12 * model::PAGE + start + 2]
        .copy_from_slice(&(selected_number as u16).to_le_bytes());
    case
}

#[test]
fn qualify_shorter_historical_guard_sources_before_preview() {
    for variable in [false, true] {
        let case = older_source(variable);
        qualify(
            &case.before,
            &case.rows,
            case.variable,
            case.selected_number,
        );
    }
}

#[test]
fn historical_fixed_and_batch_updates_match_every_prefrozen_whole_image() {
    for code in [5, 6] {
        for descending in [false, true] {
            for variable in [false, true] {
                for nullable in [false, true] {
                    for selected_number in [2, 4] {
                        let case =
                            model::case(code, descending, variable, nullable, selected_number);
                        let updates = requests(nullable);
                        let fixed = capture(&case.before)
                            .preview_ace16_fixed_updates(
                                "Items",
                                "Score",
                                &updates,
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap();
                        assert_eq!(fixed.candidate_bytes(), case.expected);
                        assert_eq!(fixed.changed_rows(), 2);

                        let groups = [FixedColumnUpdates::new("Items", "Score", &updates)];
                        let batch = capture(&case.before)
                            .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
                            .unwrap();
                        assert_eq!(batch.candidate_bytes(), case.expected);
                        assert_eq!(batch.changed_rows(), 2);
                    }
                }
            }
        }
    }
}

#[test]
fn full_width_noops_preserve_every_byte_and_shorter_sources_refuse_even_empty() {
    for variable in [false, true] {
        for selected_number in [2, 4] {
            let case = model::case(6, true, variable, true, selected_number);
            for updates in [
                Vec::new(),
                vec![
                    FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(10)),
                    FixedScalarUpdate::new(1, Value::Null, Value::Null),
                ],
            ] {
                let result = capture(&case.before)
                    .preview_ace16_fixed_updates("Items", "Score", &updates, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(result.changed_rows(), 0);
                assert_eq!(result.candidate_bytes(), case.before);
            }
        }

        let older = older_source(variable);
        qualify(
            &older.before,
            &older.rows,
            older.variable,
            older.selected_number,
        );
        for updates in [Vec::new(), requests(true)] {
            assert_eq!(
                capture(&older.before)
                    .preview_ace16_fixed_updates("Items", "Score", &updates, &mut budget(), &|| {
                        false
                    })
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout)),
            );
            let groups = [FixedColumnUpdates::new("Items", "Score", &updates)];
            assert_eq!(
                capture(&older.before)
                    .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout)),
            );
        }
    }
}

#[test]
fn historical_scope_keeps_other_admissions_and_unknown_shapes_closed() {
    let case = model::case(5, false, true, false, 4);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_deletes("Items", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout)),
    );
    assert_eq!(
        capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(case.rows[0].clone())],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout)),
    );
    assert_eq!(
        capture(&case.before)
            .preview_ace16_variable_updates("Items", "Label", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout)),
    );
    let groups = [FixedColumnUpdates::new("Items", "Score", &[])];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_cascading_updates(&groups, &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout)),
    );

    let mut jet4 = case.before.clone();
    jet4[4..20].copy_from_slice(b"Standard Jet DB\0");
    jet4[20] = 1;
    assert_eq!(
        capture(&jet4)
            .plan_int32_updates(
                "Items",
                "Score",
                &[] as &[Int32Update],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout)),
    );

    let mut unknown = case.before.clone();
    unknown[10 * model::PAGE + 75 + 25 + 15] |= 0x80;
    assert!(matches!(
        capture(&unknown).preview_ace16_fixed_updates(
            "Items",
            "Score",
            &[],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(_))
    ));
    let out_of_range = [FixedScalarUpdate::new(
        usize::MAX,
        Value::Int32(10),
        Value::Int32(17),
    )];
    assert!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Score", &out_of_range, &mut budget(), &|| false)
            .is_err()
    );
    let overflowing_carrier = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int64(i64::from(i32::MAX) + 1),
    )];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &overflowing_carrier,
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Column)),
    );
}

#[test]
fn historical_source_proof_is_paid_and_final_cancellation_publishes_no_candidate() {
    let case = model::case(6, false, true, true, 4);
    let updates = requests(true);
    let mut measured = budget();
    let available =
        [Resource::Bytes, Resource::Rows, Resource::Steps].map(|pool| measured.remaining(pool));
    let polls = Cell::new(0);
    let result = capture(&case.before)
        .preview_ace16_fixed_updates("Items", "Score", &updates, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(result.candidate_bytes(), case.expected);
    let used = [Resource::Bytes, Resource::Rows, Resource::Steps].map(|pool| {
        let position = match pool {
            Resource::Bytes => 0,
            Resource::Rows => 1,
            Resource::Steps => 2,
        };
        available[position] - measured.remaining(pool)
    });

    for pool in 0..3 {
        let mut exact = [256 * 1024 * 1024, 100_000, 100_000_000];
        exact[pool] = used[pool];
        let result = capture(&case.before)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &updates,
                &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                &|| false,
            )
            .unwrap();
        assert_eq!(result.candidate_bytes(), case.expected);

        exact[pool] -= 1;
        assert!(
            capture(&case.before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &updates,
                    &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                    &|| false
                )
                .is_err()
        );
    }

    let stop = polls.get();
    let polls = Cell::new(0);
    assert!(matches!(
        capture(&case.before).preview_ace16_fixed_updates(
            "Items",
            "Score",
            &updates,
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

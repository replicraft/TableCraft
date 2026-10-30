use super::*;
use crate::{
    ConstraintViolation, EditError, IncrementingAutoNumber, IndexConstraintViolation,
    InsertFieldValue, InspectError, OmittedRowInsert, WriteBlocker,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::FieldConstraintViolation;

fn request(row: &Row, omitted: &[usize]) -> OmittedRowInsert {
    OmittedRowInsert::new(
        row.values(true)
            .into_iter()
            .enumerate()
            .map(|(position, value)| {
                if omitted.contains(&position) {
                    InsertFieldValue::Omitted
                } else {
                    InsertFieldValue::Supplied(value)
                }
            })
            .collect(),
    )
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

#[test]
fn omitted_defaults_and_explicit_null_value_empty_match_all_literal_whole_images() {
    for code in [5, 6] {
        for case in cases(code) {
            let expected = fixture::expected(&case.source, std::slice::from_ref(&case.final_row));
            let result = original::capture(&case.source.before)
                .preview_ace16_insert_rows_with_omissions(
                    "Items",
                    &[request(&case.final_row, &case.omitted)],
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
            same(result.candidate_bytes(), &expected);
            assert_eq!(result.changed_rows(), 1);
            assert_eq!(result.before_bytes(), case.source.before);

            let empty = original::capture(&case.source.before)
                .preview_ace16_insert_rows_with_omissions(
                    "Items",
                    &[],
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
            same(empty.candidate_bytes(), &case.source.before);
        }
    }
}

#[test]
fn needed_unknown_type_required_unique_and_validation_refusals_remain_exact() {
    for code in [5, 6] {
        for (position, bytes) in refusal_sources(code).into_iter().enumerate() {
            let omitted = match position {
                7 | 10 => 2,
                8 => 1,
                9 => 13,
                _ => 0,
            };
            let error = match position {
                0..=5 => EditError::Unsupported(WriteBlocker::Properties),
                6 | 8 => EditError::Unsupported(WriteBlocker::Column),
                7 => EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey,
                )),
                9 => EditError::Inspect(InspectError::Constraint(ConstraintViolation::Field(
                    FieldConstraintViolation::Required,
                ))),
                10 => EditError::Inspect(InspectError::Constraint(ConstraintViolation::FieldRule)),
                _ => panic!("unexpected refusal source"),
            };
            assert_eq!(
                original::capture(&bytes)
                    .preview_ace16_insert_rows_with_omissions(
                        "Items",
                        &[request(&Row::rich(10), &[omitted])],
                        &mut original::budget(),
                        &|| false,
                    )
                    .err(),
                Some(error),
                "layout {code}, refusal source {position}"
            );
        }
    }
}

#[test]
fn omitted_companions_keep_explicit_last_policy_and_nonfirst_generated_primary() {
    for code in [5, 6] {
        let mut case = generated::source(code, generated::State::HighWater);
        properties::properties(
            &mut case.before,
            &[field("Score", vec![Property::text("DefaultValue", "99")])],
        );
        let row = generated::Row {
            earlier: 4,
            id: 261,
            score: Some(99),
        };
        let expected = generated::expected(&case, std::slice::from_ref(&row));
        let values = || {
            [OmittedRowInsert::new(vec![
                InsertFieldValue::Supplied(Value::Int32(4)),
                InsertFieldValue::Omitted,
            ])]
        };
        let candidate = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows_with_omissions(
                "Items",
                &values(),
                IncrementingAutoNumber::new(260),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.candidate_bytes(), &expected);
        assert_eq!(candidate.generated_column_number(), 1);
        assert_eq!(candidate.generated_values(), &[261]);
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows_with_omissions(
                    "Items",
                    &values(),
                    IncrementingAutoNumber::new(259),
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Conflict)
        );
        let empty = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows_with_omissions(
                "Items",
                &[],
                IncrementingAutoNumber::new(260),
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        same(empty.candidate_bytes(), &case.before);
    }
    let maximum = generated::source(6, generated::State::Maximum);
    checked(&maximum.before, 0, 4);
    assert_eq!(
        original::capture(&maximum.before)
            .preview_ace16_incrementing_insert_rows_with_omissions(
                "Items",
                &[OmittedRowInsert::new(vec![
                    InsertFieldValue::Supplied(Value::Int32(4)),
                    InsertFieldValue::Omitted
                ])],
                IncrementingAutoNumber::new(i32::MAX),
                &mut original::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::AutoNumberExhausted)
    );
}

#[test]
fn omission_work_uses_all_budget_pools_and_measured_terminal_cancellation() {
    let case = source(6, "Score", Some("\"fallback\""));
    let values = || [request(&Row::rich(10), &[13])];
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_insert_rows_with_omissions("Items", &values(), &mut budget, &|| {
                    false
                })
                .is_err()
        );
    }
    let polls = Cell::new(0);
    original::capture(&case.before)
        .preview_ace16_insert_rows_with_omissions(
            "Items",
            &values(),
            &mut original::budget(),
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap();
    let terminal = polls.get();
    assert!(terminal > 0);
    polls.set(0);
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_insert_rows_with_omissions(
                "Items",
                &values(),
                &mut original::budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == terminal
                }
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}

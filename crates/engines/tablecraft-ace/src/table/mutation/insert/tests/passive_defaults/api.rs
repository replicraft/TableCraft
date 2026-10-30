//! API comparisons registered only after the source/expected qualification gate.
use super::*;
use crate::{
    AutoNumberRowInsert, ConstraintViolation, EditError, InspectError, RowInsert, WriteBlocker,
};
use tablecraft_core::values::Value;
use tablecraft_schema::FieldConstraintViolation;

#[test]
fn supplied_value_null_empty_and_empty_batch_never_evaluate_or_replace_defaults() {
    for code in [5, 6] {
        for expression in [
            "\"stored default\"",
            "Date()",
            "UnresolvedDefaultFunction()",
        ] {
            let case = model::ordinary(code, expression, false);
            let additions = model::additions();
            let expected = fixture::expected(&case, &additions);
            let requests = additions
                .iter()
                .map(|row| RowInsert::new(row.values(true)))
                .collect::<Vec<_>>();
            let candidate = original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.candidate_bytes(), expected);
            assert_eq!(candidate.changed_rows(), 3);
            let empty = original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &[], &mut original::budget(), &|| false)
                .unwrap();
            assert_eq!(empty.candidate_bytes(), case.before);
        }
    }
}

#[test]
fn defaults_do_not_override_required_arity_or_explicit_carrier_checks() {
    let case = model::ordinary(6, "\"fallback\"", true);
    let mut row = Row::rich(10);
    row.score = None;
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(row.values(true))],
                &mut original::budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::Field(FieldConstraintViolation::Required,)
        )))
    );
    let case = model::ordinary(6, "\"fallback\"", false);
    for values in [vec![Value::Int32(10)], {
        let mut values = Row::rich(10).values(true);
        values[13] = Value::Binary(Vec::new());
        values
    }] {
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(values)],
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
    }
}

#[test]
fn incrementing_companions_keep_explicit_null_and_original_default_bytes() {
    for code in [5, 6] {
        let mut case = generated::source(code, generated::State::HighWater);
        model::properties(
            &mut case.before,
            &[field("Score", vec![Property::text("DefaultValue", "999")])],
        );
        let additions = [generated::Row {
            earlier: 4,
            id: 261,
            score: None,
        }];
        let expected = generated::expected(&case, &additions);
        let image = original::capture(&case.before);
        let policy = image
            .prepare_ace16_incrementing_insert_policy("Items", &mut original::budget(), &|| false)
            .unwrap();
        assert_eq!(policy.expected_last(), 260);
        let requests = [AutoNumberRowInsert::new(additions[0].companions())];
        let candidate = image
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &requests,
                policy,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
    }
}

#[test]
fn unsupported_default_scopes_flags_carriers_and_generated_or_ole_fields_stay_refused() {
    for (bytes, generated) in negative_sources() {
        let image = original::capture(&bytes);
        let error = if generated {
            image
                .prepare_ace16_incrementing_insert_policy("Items", &mut original::budget(), &|| {
                    false
                })
                .err()
        } else {
            image
                .preview_ace16_insert_rows("Items", &[], &mut original::budget(), &|| false)
                .err()
        };
        assert_eq!(
            error,
            Some(EditError::Unsupported(WriteBlocker::Properties))
        );
    }
}

#[test]
fn defaults_preserve_empty_text_and_validation_rule_constraints() {
    for code in [5, 6] {
        let case = model::rules(code, false);
        let mut empty = Row::rich(10);
        empty.score = Some(String::new());
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(empty.values(true))],
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::Field(FieldConstraintViolation::EmptyText,)
            )))
        );
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_insert_rows(
                    "Items",
                    &[RowInsert::new(Row::rich(-1).values(true))],
                    &mut original::budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::FieldRule
            )))
        );
    }
}

//! Register only after the independent source/expected read gate passes.
use super::*;
use crate::{
    AutoNumberRowInsert, EditError, FixedScalarUpdate, RowDelete, RowInsert, SlotScalarUpdate,
    WriteBlocker,
};
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[test]
fn display_properties_preserve_explicit_insert_value_null_empty_and_noop() {
    for code in [5, 6] {
        for places in [0, 2, 15, 255] {
            for flags in [0, 1] {
                let case = source(code, places, flags);
                let rows = additions();
                let expected = fixture::expected(&case, &rows);
                let requests = rows
                    .iter()
                    .map(|row| RowInsert::new(row.values(true)))
                    .collect::<Vec<_>>();
                let result = original::capture(&case.before)
                    .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_eq!(result.candidate_bytes(), expected);
                assert_eq!(result.changed_rows(), 3);

                let empty = original::capture(&case.before)
                    .preview_ace16_insert_rows("Items", &[], &mut original::budget(), &|| false)
                    .unwrap();
                assert_eq!(empty.candidate_bytes(), case.before);
                assert_eq!(empty.changed_rows(), 0);
            }
        }
    }
}

#[test]
fn display_properties_do_not_round_fixed_values_or_render_updated_text() {
    for code in [5, 6] {
        let case = source(code, 0, 1);
        let fixed = fixed_expected(&case);
        let candidate = original::capture(&case.before)
            .preview_ace16_fixed_updates(
                "Items",
                "Earlier",
                &[FixedScalarUpdate::new(
                    0,
                    Value::Int32(1001),
                    Value::Int32(777),
                )],
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), fixed);

        let text = text_expected(&case);
        let candidate = original::capture(&case.before)
            .preview_ace16_slot_updates(
                "Items",
                "Score",
                &[SlotScalarUpdate::new(
                    0,
                    Value::Text("AB".to_owned()),
                    Value::Text("mI".to_owned()),
                )],
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), text);
    }
}

#[test]
fn deleting_rows_preserves_display_declarations_and_inactive_storage() {
    for code in [5, 6] {
        let case = source(code, 255, 1);
        let expected = delete_expected(&case);
        let candidate = original::capture(&case.before)
            .preview_ace16_deletes(
                "Items",
                &[RowDelete::new(0, case.rows[0].record(true))],
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), expected);
        assert_eq!(candidate.deleted_rows(), 1);
    }
}

#[test]
fn unrelated_properties_wrong_scope_carrier_value_and_flags_remain_refused() {
    for bytes in negative_sources() {
        assert_eq!(
            original::capture(&bytes)
                .preview_ace16_insert_rows("Items", &[], &mut original::budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Properties))
        );
    }
}

#[test]
fn explicit_incrementing_policy_preserves_generated_format_without_detecting_mode() {
    for code in [5, 6] {
        let case = generated_source(code);
        let added = generated_addition();
        let expected = generated::expected(&case, std::slice::from_ref(&added));
        let image = original::capture(&case.before);
        let policy = image
            .prepare_ace16_incrementing_insert_policy("Items", &mut original::budget(), &|| false)
            .unwrap();
        assert_eq!(policy.expected_last(), 260);
        let candidate = image
            .preview_ace16_incrementing_insert_rows(
                "Items",
                &[AutoNumberRowInsert::new(added.companions())],
                policy,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.generated_values(), &[261]);
        assert_eq!(candidate.candidate_bytes(), expected);
    }
}

#[test]
fn display_qualification_keeps_budget_and_terminal_cancellation_controls() {
    let case = source(6, 15, 1);
    let request = || [RowInsert::new(additions()[0].values(true))];
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &request(), &mut budget, &|| false)
                .is_err()
        );
    }

    let polls = Cell::new(0);
    original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &request(), &mut original::budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let terminal = polls.get();
    assert!(terminal > 0);
    polls.set(0);
    assert!(
        original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &request(), &mut original::budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            })
            .is_err()
    );
}

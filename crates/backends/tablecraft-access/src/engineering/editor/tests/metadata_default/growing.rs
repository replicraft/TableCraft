//! Explicit variable placement composes one fixed/variable statement.
use super::{AccessEngineeringEditor, ScalarUpdateMode, budget, image, model, qualify};
use crate::NativeEditError;
use tablecraft_ace::{EditError, WriteBlocker};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};

#[path = "../../../../../../../engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/growing/model.rs"]
mod source_model;

fn cases() -> Vec<source_model::Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for destination in [
                    source_model::Destination::OriginalPage,
                    source_model::Destination::OtherOwned,
                    source_model::Destination::EndOfFile,
                ] {
                    cases.push(source_model::case(code, family, descending, destination));
                }
            }
        }
    }
    cases
}

#[test]
fn qualify_variable_batch_sources_and_complete_expected_before_frontend_api() {
    let cases = cases();
    assert_eq!(cases.len(), 36);
    for case in cases {
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        let slot = 12 * model::PAGE + 14 + 3 * 2;
        let word = u16::from_le_bytes(case.expected[slot..slot + 2].try_into().unwrap());
        assert_ne!(word & 0x4000, 0);
        let start = 12 * model::PAGE + usize::from(word & 0x1fff);
        assert_eq!(&case.expected[start..start + 4], &case.target.to_le_bytes());
    }
}

#[test]
fn explicit_variable_batch_matches_all_prefrozen_images_in_both_orders() {
    for case in cases() {
        for reverse in [false, true] {
            let editor = AccessEngineeringEditor::new(
                image(&case.before),
                "Items",
                ScalarUpdateMode::Variable {
                    index_growth: false,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let row = editor.snapshot().row_handle(3).unwrap();
            let age = [CellUpdate::new(row, Value::Int32(40))];
            let name = [CellUpdate::new(
                row,
                model::name_value(&case.final_rows[3], case.family),
            )];
            let mut groups = [
                ColumnUpdates {
                    column: "Age",
                    updates: &age,
                },
                ColumnUpdates {
                    column: "Name",
                    updates: &name,
                },
            ];
            if reverse {
                groups.reverse();
            }
            let copy = editor
                .plan_columns(&groups, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(copy.before_bytes(), case.before);
            assert_eq!(copy.candidate_bytes(), case.expected);
        }
    }
}

#[test]
fn default_stays_fitted_and_mixed_index_growth_pays_request_allocations() {
    let case = source_model::case(
        6,
        model::Family::Legacy,
        false,
        source_model::Destination::EndOfFile,
    );
    let editor = AccessEngineeringEditor::new(
        image(&case.before),
        "Items",
        ScalarUpdateMode::Default,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let row = editor.snapshot().row_handle(3).unwrap();
    let age = [CellUpdate::new(row, Value::Int32(40))];
    let name = [CellUpdate::new(row, Value::Text("aa".to_owned()))];
    let groups = [
        ColumnUpdates {
            column: "Age",
            updates: &age,
        },
        ColumnUpdates {
            column: "Name",
            updates: &name,
        },
    ];
    assert!(matches!(
        editor.plan_columns(&groups, &mut budget(), &|| false),
        Err(NativeEditError::Edit(EditError::Unsupported(
            WriteBlocker::RowLayout
        )))
    ));

    for selected in [false, true] {
        let editor = AccessEngineeringEditor::new(
            image(&case.before),
            "Items",
            ScalarUpdateMode::Variable { index_growth: true },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let row = editor.snapshot().row_handle(3).unwrap();
        let age = if selected {
            vec![CellUpdate::new(row, Value::Int32(40))]
        } else {
            Vec::new()
        };
        let name = if selected {
            vec![CellUpdate::new(row, Value::Text("aa".to_owned()))]
        } else {
            Vec::new()
        };
        let groups = [
            ColumnUpdates {
                column: "Age",
                updates: &age,
            },
            ColumnUpdates {
                column: "Name",
                updates: &name,
            },
        ];
        assert!(matches!(
            editor.plan_columns(
                &groups,
                &mut ResourceBudget::new(0, 100_000, 100_000_000),
                &|| false
            ),
            Err(NativeEditError::Inspect(_))
        ));
    }
}

#[test]
fn variable_batch_refuses_fixed_only_targets_even_without_selected_rows() {
    let case = source_model::case(
        6,
        model::Family::Legacy,
        false,
        source_model::Destination::OriginalPage,
    );
    let editor = AccessEngineeringEditor::new(
        image(&case.before),
        "Items",
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let groups = [
        ColumnUpdates {
            column: "Age",
            updates: &[],
        },
        ColumnUpdates {
            column: "Id",
            updates: &[],
        },
    ];
    assert!(matches!(
        editor.plan_columns(&groups, &mut budget(), &|| false),
        Err(NativeEditError::Unsupported)
    ));
}

#[test]
fn multiple_variable_empty_selections_preserve_the_qualified_source_in_both_modes() {
    let case = source_model::case(
        6,
        model::Family::Legacy,
        false,
        source_model::Destination::OriginalPage,
    );
    for index_growth in [false, true] {
        let editor = AccessEngineeringEditor::new(
            image(&case.before),
            "Items",
            ScalarUpdateMode::Variable { index_growth },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let groups = [
            ColumnUpdates {
                column: "Name",
                updates: &[],
            },
            ColumnUpdates {
                column: "Other",
                updates: &[],
            },
        ];
        let copy = editor
            .plan_columns(&groups, &mut budget(), &|| false)
            .unwrap_or_else(|error| {
                panic!("qualified empty variable mode {index_growth}: {error}")
            });
        assert_eq!(copy.before_bytes(), case.before);
        assert_eq!(copy.candidate_bytes(), case.before);
    }
}

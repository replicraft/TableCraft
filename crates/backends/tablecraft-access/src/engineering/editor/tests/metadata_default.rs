//! Frontend consumers reuse the engine's independently frozen whole-image owners.
mod boundaries;
mod growing;
use super::{AccessEngineeringEditor, ScalarUpdateMode, budget, fixture as base, image, rows};
use crate::engineering::{UpdateMode, sql};
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, check_table, check_table_constraints};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

#[path = "../../../../../../engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/capacity.rs"]
mod capacity;
#[path = "../../../../../../engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/model.rs"]
mod model;

fn qualify(bytes: &[u8], expected: &[model::Row], family: model::Family) {
    assert_eq!(
        rows(bytes),
        expected
            .iter()
            .map(|row| model::values(row, family))
            .collect::<Vec<_>>()
    );
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 4);
    assert_eq!(checked.indexes().len(), 6);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 4);
}

fn preservation(case: &model::Case) {
    assert_eq!(case.before.len(), case.expected.len());
    assert_eq!(case.records.len(), case.final_records.len());
    assert_eq!(case.records.len(), 4);
    assert!(
        case.records
            .iter()
            .zip(&case.final_records)
            .all(|(before, after)| before.len() == after.len())
    );
    assert_eq!(
        &case.before[..12 * model::PAGE],
        &case.expected[..12 * model::PAGE]
    );
    if case.forwarded {
        assert_eq!(case.before.len(), 18 * model::PAGE);
    }
}

fn cases() -> Vec<model::Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [
            model::Family::Legacy,
            model::Family::Modern,
            model::Family::Binary,
        ] {
            for descending in [false, true] {
                for recipe in [
                    model::Recipe::Permutation,
                    model::Recipe::Shorter,
                    model::Recipe::Nulls,
                ] {
                    cases.push(model::case(code, family, descending, recipe, false));
                }
            }
        }
    }
    for family in [
        model::Family::Legacy,
        model::Family::Modern,
        model::Family::Binary,
    ] {
        cases.push(model::case(
            6,
            family,
            true,
            model::Recipe::Permutation,
            true,
        ));
    }
    for family in [model::Family::Legacy, model::Family::Modern] {
        for descending in [false, true] {
            cases.push(capacity::case(family, descending));
        }
    }
    cases
}

#[test]
fn qualify_metadata_default_sources_and_complete_expected_before_frontend_api() {
    let cases = cases();
    assert_eq!(cases.len(), 43);
    for case in cases {
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        preservation(&case);
    }
}

#[test]
fn typed_default_joint_updates_match_all_frozen_images_in_both_group_orders() {
    for case in cases() {
        for reverse in [false, true] {
            let editor = AccessEngineeringEditor::new(
                image(&case.before),
                "Items",
                ScalarUpdateMode::Default,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let snapshot = editor.snapshot();
            let age = case
                .changed
                .iter()
                .map(|ordinal| {
                    CellUpdate::new(
                        snapshot.row_handle(*ordinal).unwrap(),
                        case.final_rows[*ordinal]
                            .age
                            .map_or(Value::Null, Value::Int32),
                    )
                })
                .collect::<Vec<_>>();
            let name = case
                .changed
                .iter()
                .map(|ordinal| {
                    CellUpdate::new(
                        snapshot.row_handle(*ordinal).unwrap(),
                        model::name_value(&case.final_rows[*ordinal], case.family),
                    )
                })
                .collect::<Vec<_>>();
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
fn default_sql_uses_joint_original_values_and_explicit_fixed_stays_strict() {
    for family in [model::Family::Legacy, model::Family::Modern] {
        for descending in [false, true] {
            for case in [
                model::case(6, family, descending, model::Recipe::Permutation, false),
                capacity::case(family, descending),
            ] {
                let query = parse_query(
                    "UPDATE Items SET Age=IIf(Id=1,20,10), Name=IIf(Id=1,'é','a') WHERE Id IN(1,2)",
                    ParseLimits::default(),
                )
                .unwrap();
                let plan = sql::preview_batch_update(
                    image(&case.before),
                    query.query(),
                    ScalarUpdateMode::Default,
                    &mut budget(),
                    None,
                    &|| false,
                )
                .unwrap();
                assert_eq!(plan.into_backend().candidate_bytes(), case.expected);
                assert!(
                    sql::preview_batch_update(
                        image(&case.before),
                        query.query(),
                        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                        &mut budget(),
                        None,
                        &|| false
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn default_rejects_two_variable_targets_even_for_zero_matches() {
    let query = parse_query(
        "UPDATE Items SET Name='x', Other='y' WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    let case = model::case(
        6,
        model::Family::Legacy,
        false,
        model::Recipe::Permutation,
        false,
    );
    assert!(
        sql::preview_batch_update(
            image(&case.before),
            query.query(),
            ScalarUpdateMode::Default,
            &mut budget(),
            None,
            &|| false
        )
        .is_err()
    );
}

#[test]
fn default_all_fixed_keeps_existing_history_and_admission_profile() {
    let before = super::source();
    let query = parse_query("UPDATE Items SET Score=Score+1", ParseLimits::default()).unwrap();
    let default = sql::preview_batch_update(
        image(&before),
        query.query(),
        ScalarUpdateMode::Default,
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap()
    .into_backend();
    let fixed = sql::preview_batch_update(
        image(&before),
        query.query(),
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap()
    .into_backend();
    assert_eq!(default.candidate_bytes(), fixed.candidate_bytes());
    assert_eq!(default.before_bytes(), before);
}

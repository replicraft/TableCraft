//! Independent SOURCE and expected images, registered before batch SQL admission.
use super::{budget, fixture, image, rows, support};
use crate::engineering::{ScalarUpdateMode, UpdateMode, sql};
use tablecraft_sql::{ParseLimits, parse_query};

use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, check_table, check_table_constraints};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};
use tablecraft_model::CatalogScope;

mod model;
use model::{Recipe, case};

fn qualify(bytes: &[u8], values: &[[Option<i32>; 2]; 3]) {
    let table = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), 3);
    assert_eq!(table.indexes().len(), 4);
    assert!(
        table
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
    assert_eq!(constraints.rows(), 3);
    for (ordinal, row) in rows(bytes).iter().enumerate() {
        assert_eq!(
            row,
            &vec![
                Value::Int32(ordinal as i32 + 1),
                values[ordinal][0].map_or(Value::Null, Value::Int32),
                values[ordinal][1].map_or(Value::Null, Value::Int32),
            ]
        );
    }
}

#[test]
fn qualify_nullable_nonprimary_tuple_sources_and_expected_images_before_sql_batch_api() {
    for code in [5, 6] {
        for recipe in [Recipe::Clear, Recipe::Exchange, Recipe::Introduce] {
            let example = case(code, recipe);
            qualify(&example.before, &example.original);
            qualify(&example.expected, &example.final_values);
            assert_eq!(example.before.len(), example.expected.len());
            assert_eq!(&example.before[..10 * 4096], &example.expected[..10 * 4096]);
            assert_eq!(&example.before[15 * 4096..], &example.expected[15 * 4096..]);
        }
    }
}

#[test]
fn one_finish_merges_shared_mask_and_both_physical_alias_groups() {
    for code in [5, 6] {
        for (recipe, text) in [
            (
                Recipe::Clear,
                "UPDATE Items SET [Group]=NULL, Score=NULL WHERE Id=1",
            ),
            (
                Recipe::Exchange,
                "UPDATE Items SET [Group]=Score, Score=[Group]",
            ),
            (
                Recipe::Introduce,
                "UPDATE Items SET [Group]=20, Score=200 WHERE Id=3",
            ),
        ] {
            let example = case(code, recipe);
            let query = parse_query(text, ParseLimits::default()).unwrap();
            let plan = sql::preview_batch_update(
                image(&example.before),
                query.query(),
                ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                &mut budget(),
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(
                plan.matched_rows(),
                if matches!(recipe, Recipe::Exchange) {
                    3
                } else {
                    1
                }
            );
            let copy = plan.into_backend();
            assert_eq!(copy.candidate_bytes(), example.expected);
            qualify(copy.candidate_bytes(), &example.final_values);
        }
    }
}

#[test]
fn duplicate_targets_and_multiple_variable_columns_refuse_without_a_candidate() {
    let example = case(5, Recipe::Exchange);
    for text in [
        "UPDATE Items AS i SET i.Score=1, score=2 WHERE FALSE",
        "UPDATE Items SET [Group]=10, Score=2147483648 WHERE Id=3",
    ] {
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(
            sql::preview_batch_update(
                image(&example.before),
                query.query(),
                ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                &mut budget(),
                None,
                &|| false,
            )
            .is_err()
        );
    }
    let variable = super::source::source();
    for mode in [
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        ScalarUpdateMode::LongValue,
    ] {
        for filter in ["", " WHERE FALSE"] {
            let text = format!("UPDATE Items SET Label='new', Other='other'{filter}");
            let query = parse_query(&text, ParseLimits::default()).unwrap();
            assert!(matches!(
                sql::preview_batch_update(
                    image(&variable),
                    query.query(),
                    mode,
                    &mut budget(),
                    None,
                    &|| false,
                ),
                Err(tablecraft_query::UpdateError::Backend(
                    crate::NativeEditError::Unsupported
                ))
            ));
        }
    }
}

#[test]
fn direct_batch_editor_rejects_cross_snapshot_handles_and_repeated_groups() {
    let example = case(5, Recipe::Exchange);
    let first = crate::engineering::AccessEngineeringEditor::new(
        image(&example.before),
        "Items",
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let foreign = first.snapshot().row_handle(0).unwrap();
    let second = crate::engineering::AccessEngineeringEditor::new(
        image(&example.before),
        "Items",
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let updates = [CellUpdate::new(foreign, Value::Int32(20))];
    assert!(matches!(
        second.plan_columns(
            &[ColumnUpdates {
                column: "Group",
                updates: &updates
            }],
            &mut budget(),
            &|| false,
        ),
        Err(crate::NativeEditError::Snapshot(_))
    ));

    let editor = crate::engineering::AccessEngineeringEditor::new(
        image(&example.before),
        "Items",
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        editor.plan_columns(
            &[
                ColumnUpdates {
                    column: "Group",
                    updates: &[]
                },
                ColumnUpdates {
                    column: "group",
                    updates: &[]
                }
            ],
            &mut budget(),
            &|| false,
        ),
        Err(crate::NativeEditError::Unsupported)
    ));
}

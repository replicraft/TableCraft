//! Actual typed/SQL consumers are registered only after SOURCE qualification.
use super::super::{AccessEngineeringEditor, ScalarUpdateMode};
use super::{budget, matrix, model};
use crate::engineering::{UpdateMode, sql};
use std::io::Cursor;
use tablecraft_ace::NativeImage;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn mode(case: &model::Case) -> ScalarUpdateMode {
    match case.placement {
        model::Placement::Fitted => ScalarUpdateMode::Default,
        model::Placement::Data => ScalarUpdateMode::Variable {
            index_growth: false,
        },
        model::Placement::DataIndexes => ScalarUpdateMode::Variable { index_growth: true },
    }
}

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let differences = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        actual == expected,
        "whole image lengths {}/{}, first {first:?}, differences {differences}",
        actual.len(),
        expected.len()
    );
}

#[test]
fn typed_multiple_variable_modes_match_all44_complete_pairs_in_both_group_orders() {
    for case in matrix::cases() {
        for reverse in [false, true] {
            let editor = AccessEngineeringEditor::new(
                image(&case.before),
                "Items",
                mode(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut owned = Vec::new();
            for (column, name) in [(1, "Age"), (2, "First"), (3, "Second"), (4, "Opaque")] {
                let ordinals = if column == 1 {
                    case.fixed_rows.clone()
                } else if column == 2 {
                    case.first_rows.clone()
                } else if column == 3 {
                    case.second_rows.clone()
                } else {
                    (0..case.rows.len())
                        .filter(|&row| case.rows[row].opaque != case.final_rows[row].opaque)
                        .collect()
                };
                if ordinals.is_empty() && column != 2 && column != 3 {
                    continue;
                }
                let cells = ordinals
                    .into_iter()
                    .map(|row| {
                        CellUpdate::new(
                            editor.snapshot().row_handle(row).unwrap(),
                            model::values(&case.final_rows[row], case.family)[column].clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                owned.push((name, cells));
            }
            let mut groups = owned
                .iter()
                .map(|(name, cells)| ColumnUpdates {
                    column: name,
                    updates: cells,
                })
                .collect::<Vec<_>>();
            if reverse {
                groups.reverse();
            }
            let copy = editor
                .plan_columns(&groups, &mut budget(), &|| false)
                .unwrap();
            same_image(copy.before_bytes(), &case.before);
            same_image(copy.candidate_bytes(), &case.expected);
        }
    }
}

#[test]
fn sql_two_text_fields_use_original_rows_and_joint_fitted_keys() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (recipe, statement) in [
                (
                    model::Recipe::Permutation,
                    "UPDATE Items SET [First]=IIf(Id=1,'s','a'), [Second]=IIf(Id=1,'s','a') WHERE Id IN(1,2)",
                ),
                (
                    model::Recipe::NullShrinkGrow,
                    "UPDATE Items SET [First]=IIf(Id=3,'',IIf(Id=4,'a',[First])), [Second]=IIf(Id=3,NULL,IIf(Id=4,'aa',IIf(Id=6,'',[Second]))), Age=IIf(Id=3,22,IIf(Id=5,NULL,Age)) WHERE Id IN(3,4,5,6)",
                ),
            ] {
                let case = model::fitted(code, model::Family::LegacyText, descending, recipe);
                let query = parse_query(statement, ParseLimits::default()).unwrap();
                let result = sql::preview_batch_update(
                    image(&case.before),
                    query.query(),
                    ScalarUpdateMode::Default,
                    &mut budget(),
                    None,
                    &|| false,
                )
                .unwrap();
                same_image(result.into_backend().candidate_bytes(), &case.expected);
            }
        }
    }
}

#[test]
fn explicit_fixed_and_lval_modes_still_refuse_multiple_variable_targets() {
    let case = model::fitted(
        6,
        model::Family::LegacyText,
        false,
        model::Recipe::Permutation,
    );
    let query = parse_query(
        "UPDATE Items SET [First]='x', [Second]='y' WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    for mode in [
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth),
        ScalarUpdateMode::LongValue,
    ] {
        assert!(
            sql::preview_batch_update(
                image(&case.before),
                query.query(),
                mode,
                &mut budget(),
                None,
                &|| false
            )
            .is_err()
        );
    }
    let fixed = parse_query(
        "UPDATE Items SET Age=1, Id=2 WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(
        sql::preview_batch_update(
            image(&case.before),
            fixed.query(),
            ScalarUpdateMode::Variable {
                index_growth: false
            },
            &mut budget(),
            None,
            &|| false
        )
        .is_err()
    );

    // Empty selection still chooses and qualifies one actual whole-row service.
    let result = sql::preview_batch_update(
        image(&case.before),
        query.query(),
        ScalarUpdateMode::Default,
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap();
    same_image(result.into_backend().candidate_bytes(), &case.before);
}

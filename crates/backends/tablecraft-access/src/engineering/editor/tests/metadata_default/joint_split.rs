//! The original joint-growth literals qualify before frontend calls.
use super::{AccessEngineeringEditor, ScalarUpdateMode, base, budget, image, rows};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, check_table, check_table_constraints, inspect_table_index_counter,
};
use tablecraft_core::values::Value;
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable};
use tablecraft_model::CatalogScope;

#[path = "../../../../../../../engines/tablecraft-ace/src/table/mutation/fitted/growing/joint_split_tests/model.rs"]
pub(in crate::engineering::editor::tests) mod source_model;
use source_model::{Family, Shape};

fn cases() -> Vec<source_model::Case> {
    let mut cases = Vec::new();
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for shape in [Shape::Root, Shape::Branch] {
                    cases.push(source_model::case(code, family, descending, shape));
                }
            }
        }
    }
    cases
}

fn name(row: &source_model::Row, family: Family) -> Value {
    match &row.name {
        None => Value::Null,
        Some(bytes) if matches!(family, Family::Binary) => Value::Binary(bytes.clone()),
        Some(bytes) => Value::Text(String::from_utf8(bytes.clone()).unwrap()),
    }
}

fn qualify(bytes: &[u8], expected: &[source_model::Row], family: Family) {
    assert_eq!(
        rows(bytes),
        expected
            .iter()
            .map(|row| vec![
                Value::Int32(row.id),
                Value::Int32(row.age),
                name(row, family)
            ])
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
    assert_eq!(checked.rows(), 64);
    assert_eq!(checked.indexes().len(), 4);
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            counter.value(),
            if index.index().data_number() == 0 {
                701
            } else {
                901
            }
        );
    }
    assert_eq!(
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        64
    );
}

#[test]
fn qualify_joint_index_frontend_sources_and_complete_expected_before_actions() {
    let cases = cases();
    assert_eq!(cases.len(), 24);
    for case in cases {
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        assert_eq!(&case.before[..4096], &case.expected[..4096]);
        assert!(!case.data_pages.is_empty());
        assert!(!case.index_pages.is_empty());
        assert!(
            case.data_pages
                .iter()
                .all(|page| !case.index_pages.contains(page))
        );
        assert_eq!(
            case.expected.len(),
            case.before.len() + (case.data_pages.len() + case.index_pages.len()) * 4096
        );
    }
}

#[test]
fn joint_index_typed_batches_match_all_24_images_in_both_orders() {
    for case in cases() {
        for reverse in [false, true] {
            let editor = AccessEngineeringEditor::new(
                image(&case.before),
                "Items",
                ScalarUpdateMode::Variable { index_growth: true },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let age = case
                .final_rows
                .iter()
                .enumerate()
                .map(|(ordinal, row)| {
                    CellUpdate::new(
                        editor.snapshot().row_handle(ordinal).unwrap(),
                        Value::Int32(row.age),
                    )
                })
                .collect::<Vec<_>>();
            let names = case
                .final_rows
                .iter()
                .enumerate()
                .map(|(ordinal, row)| {
                    CellUpdate::new(
                        editor.snapshot().row_handle(ordinal).unwrap(),
                        name(row, case.family),
                    )
                })
                .collect::<Vec<_>>();
            let mut columns = [
                ColumnUpdates {
                    column: "Age",
                    updates: &age,
                },
                ColumnUpdates {
                    column: "Name",
                    updates: &names,
                },
            ];
            if reverse {
                columns.reverse();
            }
            let copy = editor
                .plan_columns(&columns, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(copy.before_bytes(), case.before);
            assert_eq!(copy.candidate_bytes(), case.expected);
        }
    }
}

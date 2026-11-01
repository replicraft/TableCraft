//! Public normal TCND commits compare only with the read-qualified literal images.
#[path = "genuine_indexed_delete/fixtures.rs"]
mod fixtures;
#[path = "genuine_indexed_delete/recovery.rs"]
mod recovery;

use super::{Storage, budget, model as journals, scope};
use fixtures::Recipe;
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, NativeImage, NativeIntentError, RecoverableEditError, RowDelete,
    check_table, inspect_table_index_counter, read_table, read_table_index,
};
use tablecraft_testkit::access::genuine_indexed_delete as sources;

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires all sixteen independently qualified indexed DELETE source images"]
fn indexed_delete_commits_both_trees_aliases_and_exact_original_tcnd_orders() {
    let mut commits = 0;
    for case in fixtures::cases() {
        let orders: &[bool] = if case.recipe == Recipe::Exterior {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let requests = fixtures::requests(&case, reverse);
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes(
                    "Items",
                    &requests.iter().map(|row| row.row()).collect::<Vec<_>>(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            // Preparation returns source row order; supplied intent order stays
            // explicit and is checked below, independently of that convenience.
            let mut actual = prepared.iter().map(|row| row.row()).collect::<Vec<_>>();
            actual.sort_unstable();
            assert_eq!(actual, case.recipe.selected());
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes("Items", requests, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), case.recipe.selected().len());
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                fixtures::wire(&case, reverse)
            );
            let expected = sources::witnesses(case.recipe, reverse);
            for (request, (ordinal, record)) in plan.intent().deletes().iter().zip(&expected) {
                assert_eq!(request.row(), *ordinal);
                assert_eq!(request.expected_bytes(), record);
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                expected.len()
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            let checked = check_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 3 - expected.len());
            assert_eq!(checked.indexes().len(), 4);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            for (name, physical) in [
                ("PrimaryID", 0),
                ("PrimaryAlias", 0),
                ("GroupOrder", 1),
                ("GroupAlias", 1),
            ] {
                let rows = read_table_index(
                    &mut Cursor::new(target.bytes.get_ref()),
                    scope(),
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(rows.rows(), sources::final_rows(case.recipe));
                let history = inspect_table_index_counter(
                    &mut Cursor::new(target.bytes.get_ref()),
                    scope(),
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(history.physical_index(), physical);
                assert_eq!(history.value(), 3);
            }
            let rows = read_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), sources::final_rows(case.recipe));
            commits += 1;
        }
    }
    assert_eq!(commits, 10);
}

#[test]
#[ignore = "requires all sixteen independently qualified indexed DELETE source images"]
fn indexed_delete_empty_duplicate_missing_and_stale_witness_controls_publish_no_partial_plan() {
    for case in fixtures::cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::First)
    {
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_deletes("Items", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.before_bytes(), case.before);
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.deleted_rows(), 0);
        let raw = sources::witnesses(Recipe::First, false).remove(0).1;
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_deletes(
                "Items",
                vec![
                    RowDelete::new(0, raw.clone()),
                    RowDelete::new(0, raw.clone())
                ],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Intent(NativeIntentError::Request(
                EditError::Conflict
            )))
        ));
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_deletes(
                "Items",
                vec![RowDelete::new(3, raw.clone())],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
        let mut stale = raw;
        stale[2] ^= 1;
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_deletes(
                "Items",
                vec![RowDelete::new(0, stale)],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
    }
}

#[path = "genuine_indexed_delete/resources.rs"]
mod resources;

#[path = "genuine_indexed_delete/admission.rs"]
mod admission;

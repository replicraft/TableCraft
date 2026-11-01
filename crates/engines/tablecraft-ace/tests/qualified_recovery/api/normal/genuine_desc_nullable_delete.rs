//! Producer assertions use only independently native/SDK-qualified literal AFTERs.
#[path = "genuine_desc_nullable_delete/fixtures.rs"]
mod fixtures;
#[path = "genuine_desc_nullable_delete/recovery.rs"]
mod recovery;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, NativeImage, RecoverableEditError, RowDelete, check_table,
    inspect_table_index_counter, read_table, read_table_index,
};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires native source16 and independent SDK80 DESC/NULL qualification"]
fn descending_nullable_delete_commits_literal_images_and_both_original_request_orders() {
    let mut committed = 0;
    for case in fixtures::cases() {
        let orders: &[bool] = if case.recipe == "exterior" {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let requests = fixtures::requests(&case, reverse);
            let original_order = fixtures::ordinals(&case, reverse);
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes("Items", &original_order, &mut budget(), &|| false)
                .unwrap();
            let mut captured = prepared.iter().map(RowDelete::row).collect::<Vec<_>>();
            captured.sort_unstable();
            assert_eq!(captured, case.selected);
            for request in &prepared {
                assert_eq!(request.expected_bytes(), fixtures::record(request.row()));
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes("Items", requests, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), case.selected.len());
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                fixtures::wire(&case, reverse)
            );
            for (request, ordinal) in plan.intent().deletes().iter().zip(&original_order) {
                assert_eq!(request.row(), *ordinal);
                assert_eq!(request.expected_bytes(), fixtures::record(*ordinal));
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                case.selected.len()
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
            assert_eq!(checked.rows(), 3 - case.selected.len());
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
                assert_eq!(rows.rows(), fixtures::rows(&case, physical == 1));
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
            assert_eq!(rows.rows(), fixtures::rows(&case, false));
            committed += 1;
        }
        if case.recipe == "null-first" {
            let empty = image(&case.before)
                .plan_recoverable_ordinary_row_deletes("Items", vec![], &mut budget(), &|| false)
                .unwrap();
            assert_eq!(empty.deleted_rows(), 0);
            assert_eq!(empty.before_bytes(), case.before);
            assert_eq!(empty.after_bytes(), case.before);
            let mut stale = fixtures::record(0);
            stale[10] = 1; // Presence stays NULL; raw expected witness must still match completely.
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
    assert_eq!(committed, 10);
}

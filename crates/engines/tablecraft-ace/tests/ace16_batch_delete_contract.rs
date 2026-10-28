#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;

#[path = "ace16_batch_delete_contract/trees.rs"]
mod trees;

use model::{budget, capture, requests, table};
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, TableRowDeletes, read_table};
use tablecraft_model::CatalogScope;

fn rows(bytes: &[u8], name: &str) -> Vec<Vec<tablecraft_core::values::Value>> {
    read_table(
        &mut std::io::Cursor::new(bytes),
        CatalogScope::new(1),
        name,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

#[test]
fn explicit_child_and_parent_batch_uses_one_original_and_one_final_membership_check() {
    for code in [5, 6] {
        for reverse in [false, true] {
            let model = model::database(code, false, false, true);
            let children = requests(&model, 0, &[0, 1]);
            let parents = requests(&model, 1, &[0]);
            let mut groups = [
                TableRowDeletes::new(table(0), &children),
                TableRowDeletes::new(table(1), &parents),
            ];
            if reverse {
                groups.reverse();
            }

            let candidate = capture(&model.bytes)
                .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
                .unwrap();

            assert_eq!(candidate.before_bytes(), model.bytes);
            assert_eq!(candidate.deleted_rows(), 3);
            assert_eq!(
                rows(candidate.candidate_bytes(), table(0)),
                model.rows[0][2..]
            );
            assert_eq!(
                rows(candidate.candidate_bytes(), table(1)),
                model.rows[1][1..]
            );
            assert_eq!(rows(candidate.candidate_bytes(), table(2)), model.rows[2]);
        }
    }
}

#[test]
fn a_surviving_second_child_table_blocks_the_entire_batch() {
    let model = model::database(6, false, true, false);
    let children = requests(&model, 0, &[0, 1, 2, 3]);
    let parents = requests(&model, 1, &[0, 1]);
    let groups = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &parents),
    ];

    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&groups, &mut budget(), &|| false)
            .err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::MissingParent
        )))
    );

    let other = requests(&model, 2, &[0]);
    let complete = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &parents),
        TableRowDeletes::new(table(2), &other),
    ];
    let candidate = capture(&model.bytes)
        .preview_ace16_batch_deletes(&complete, &mut budget(), &|| false)
        .unwrap();

    assert_eq!(candidate.deleted_rows(), 7);
    assert!(rows(candidate.candidate_bytes(), table(0)).is_empty());
    assert_eq!(
        rows(candidate.candidate_bytes(), table(1)),
        model.rows[1][2..]
    );
    assert!(rows(candidate.candidate_bytes(), table(2)).is_empty());
}

#[test]
fn empty_duplicate_stale_and_cancelled_groups_do_not_publish_partial_batches() {
    let model = model::database(6, false, false, false);
    let children = requests(&model, 0, &[0, 1]);
    let stale = [tablecraft_ace::RowDelete::new(0, vec![0])];
    let duplicate = [
        TableRowDeletes::new("Items", &children),
        TableRowDeletes::new("items", &[]),
    ];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&duplicate, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );

    let bad = [
        TableRowDeletes::new(table(0), &children),
        TableRowDeletes::new(table(1), &stale),
    ];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&bad, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );

    let empty = [TableRowDeletes::new("missing", &[])];
    assert!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&empty, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        capture(&model.bytes)
            .preview_ace16_batch_deletes(&[], &mut budget(), &|| true)
            .is_err()
    );

    let noop = capture(&model.bytes)
        .preview_ace16_batch_deletes(&[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(noop.deleted_rows(), 0);
    assert_eq!(noop.candidate_bytes(), model.bytes);
}

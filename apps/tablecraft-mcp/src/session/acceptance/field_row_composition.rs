//! Fresh protocol hosts must preserve retained name bytes during later row edits.
use crate::session::tests::{acknowledge, opened, save};
use serde_json::json;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting::first_number_row,
    canonical_initial_table_creation::deletes::only_long_row,
    saved_field_rename::fitting::{self, SourceCase},
};

#[test]
fn fitted_field_rename_files_allow_fresh_host_row_crud_without_tail_loss() {
    for column in ["TaxAmount", "X"] {
        let [_, renamed] = fitting::pair(SourceCase::NumberFirst, column);
        let expected_insert = first_number_row(&renamed, &[(0, Some(1)), (1, Some(0))]);
        let expected_update = first_number_row(&renamed, &[(0, Some(1)), (1, Some(42))]);
        let expected_delete = only_long_row(&expected_update);
        let (_directory, path, mut session) = opened(&renamed);
        let revision = session.revision.unwrap();
        let draft = session
            .execute(
                "begin_insert",
                json!({"document_revision": revision, "table": "Table1"}),
            )
            .unwrap();
        let review = session
            .execute(
                "review_insert",
                json!({
                    "document_revision": revision,
                    "draft_id": draft["draft_id"],
                    "inputs": [{"omitted": true}, {"omitted": true}]
                }),
            )
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), renamed);
        let save_id = save(&mut session, "apply_insert", review);
        assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
        acknowledge(&mut session, save_id, &path);
        assert_ne!(session.revision.unwrap(), revision);

        let revision = session.revision.unwrap();
        let draft = session
            .execute(
                "begin_edit",
                json!({
                    "document_revision": revision,
                    "table": "Table1",
                    "column": column,
                    "parent_ordinal": 0
                }),
            )
            .unwrap();
        let review = session
            .execute(
                "review_edit",
                json!({
                    "document_revision": revision,
                    "draft_id": draft["draft_id"],
                    "text": "42",
                    "is_null": false
                }),
            )
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
        let save_id = save(&mut session, "apply_edit", review);
        assert_eq!(std::fs::read(&path).unwrap(), expected_update);
        acknowledge(&mut session, save_id, &path);

        let rows = session
            .execute(
                "read_rows",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "table": "Table1",
                    "offset": 0
                }),
            )
            .unwrap();
        assert_eq!(rows["rows"].as_array().unwrap().len(), 1);
        assert_eq!(rows["rows"][0][0]["text"], "1");
        assert_eq!(rows["rows"][0][1]["text"], "42");
        assert_eq!(rows["columns"][1]["name"], column);

        let review = session
            .execute(
                "review_delete",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "table": "Table1",
                    "parent_ordinal": 0
                }),
            )
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), expected_update);
        let save_id = save(&mut session, "apply_delete", review);
        assert_eq!(std::fs::read(&path).unwrap(), expected_delete);
        acknowledge(&mut session, save_id, &path);
        let rows = session
            .execute(
                "read_rows",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "table": "Table1",
                    "offset": 0
                }),
            )
            .unwrap();
        assert!(rows["rows"].as_array().unwrap().is_empty());
        assert_eq!(rows["columns"][1]["name"], column);
    }
}

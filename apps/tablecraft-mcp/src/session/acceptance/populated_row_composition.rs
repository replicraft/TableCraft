//! A fresh protocol host edits actual rows after a populated metadata rename.
use crate::session::{
    Session,
    tests::{acknowledge, opened, save},
};
use serde_json::{Value, json};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_initial_table_creation::deletes::long_row, saved_field_rename::fitting,
};

#[test]
fn populated_renamed_field_keeps_rows_and_generated_history_across_protocol_edits() {
    for column in ["TaxAmount", "X"] {
        let [_, renamed] = fitting::populated_number_pair(column, &[Some(42), None, Some(-7)]);
        let (_directory, path, mut session) = opened(&renamed);
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
                    "text": "99",
                    "is_null": false
                }),
            )
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), renamed);
        let id = save(&mut session, "apply_edit", review);
        acknowledge(&mut session, id, &path);
        let [_, expected_update] =
            fitting::populated_number_pair(column, &[Some(99), None, Some(-7)]);
        assert_eq!(std::fs::read(&path).unwrap(), expected_update);
        assert_rows(
            &mut session,
            column,
            &["1", "2", "3"],
            &["99", "NULL", "-7"],
        );

        let revision = session.revision.unwrap();
        let draft = session
            .execute(
                "begin_insert",
                json!({"document_revision": revision, "table": "Table1"}),
            )
            .unwrap();
        let before = std::fs::read(&path).unwrap();
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
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let id = save(&mut session, "apply_insert", review);
        acknowledge(&mut session, id, &path);
        let [_, expected_insert] =
            fitting::populated_number_pair(column, &[Some(99), None, Some(-7), Some(0)]);
        assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
        assert_rows(
            &mut session,
            column,
            &["1", "2", "3", "4"],
            &["99", "NULL", "-7", "0"],
        );

        let before = std::fs::read(&path).unwrap();
        let review = session
            .execute(
                "review_delete",
                json!({
                    "document_revision": session.revision.unwrap(),
                    "table": "Table1",
                    "parent_ordinal": 1
                }),
            )
            .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let id = save(&mut session, "apply_delete", review);
        acknowledge(&mut session, id, &path);
        let expected_delete = long_row(&expected_insert, 1);
        assert_eq!(std::fs::read(&path).unwrap(), expected_delete);
        assert_rows(&mut session, column, &["1", "3", "4"], &["99", "-7", "0"]);
        assert!(!journal_path(&path).exists());
        drop(session);

        let mut reopened = reopen(&path);
        assert_eq!(std::fs::read(&path).unwrap(), expected_delete);
        assert_rows(&mut reopened, column, &["1", "3", "4"], &["99", "-7", "0"]);
        assert!(!journal_path(&path).exists());
    }
}

fn assert_rows(session: &mut Session, column: &str, ids: &[&str], values: &[&str]) {
    let page = session
        .execute(
            "read_rows",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Table1",
                "offset": 0
            }),
        )
        .unwrap();
    assert_eq!(page["columns"][1]["name"], column);
    let rows = page["rows"].as_array().unwrap();
    assert_eq!(rows.len(), ids.len());
    assert_eq!(ids.len(), values.len());
    for ((row, id), value) in rows.iter().zip(ids).zip(values) {
        assert_eq!(row[0]["text"], *id);
        assert_eq!(row[1]["text"], *value);
        if *value == "NULL" {
            assert_eq!(row[1]["semantics"]["is_null"], true);
        }
    }
    assert_eq!(page["has_more"], Value::Bool(false));
}

fn reopen(path: &Path) -> Session {
    let mut session = Session::new().unwrap();
    session
        .execute("open_database", json!({"path": path}))
        .unwrap();
    session
}

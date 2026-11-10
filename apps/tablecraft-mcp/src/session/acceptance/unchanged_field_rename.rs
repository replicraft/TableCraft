use crate::session::{
    Session,
    cancellation_tests::cancel_after_dispatch,
    tests::{acknowledge, opened, save},
};
use serde_json::{Value, json};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{self, SourceCase},
    saved_field_rename,
};

fn review(session: &mut Session, replacement: &str) -> Value {
    session
        .execute(
            "review_field_rename",
            json!({
                "document_revision": session.revision.unwrap(),
                "table": "Table1",
                "column": "Score",
                "native_number": 1,
                "new_name": replacement
            }),
        )
        .unwrap()
}

#[test]
fn same_name_review_reopens_exact_source_without_any_native_save_or_journal() {
    let original = after_off::source(SourceCase::Number);
    let (_directory, path, mut session) = opened(&original);
    let review = review(&mut session, "Score");
    assert_eq!(review["changed"], false);
    let result = session.execute("apply_field_rename", review).unwrap();
    assert_eq!(result["changed"], false);
    assert_eq!(result["native_write"], false);
    assert_eq!(result["verified_reopen"], true);
    assert!(result.get("save_id").is_none());
    assert!(session.active_save.is_none());
    assert!(!session.saves.is_busy());
    assert!(session.unchanged_field_rename.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}

#[test]
fn populated_unchanged_field_rename_rechecks_complete_rows_without_save_authority() {
    let values = [Some(42), None, Some(-17)];
    let [original, expected] = saved_field_rename::fitting::populated_number_pair("Score", &values);
    assert_eq!(original, expected);
    let (_directory, path, mut session) = opened(&original);
    let review = review(&mut session, "Score");
    assert_eq!(review["changed"], false);
    let result = session.execute("apply_field_rename", review).unwrap();
    assert_eq!(result["changed"], false);
    assert_eq!(result["native_write"], false);
    assert_eq!(result["verified_reopen"], true);
    assert!(result.get("save_id").is_none());
    assert!(session.active_save.is_none());
    assert!(!session.saves.is_busy());
    assert!(session.unchanged_field_rename.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
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
    let rows = page["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][1]["text"], "42");
    assert_eq!(rows[1][1]["semantics"]["is_null"], true);
    assert_eq!(rows[2][1]["text"], "-17");
}

#[test]
fn failed_exact_reopen_retains_opaque_owner_until_verified_retry_or_explicit_discard() {
    for discard in [false, true] {
        let [original, different] = saved_field_rename::pair(SourceCase::Number);
        let (_directory, path, mut session) = opened(&original);
        let review = review(&mut session, "Score");
        let id = review["review_id"].as_u64().unwrap();
        std::fs::write(&path, &different).unwrap();
        assert!(session.execute("apply_field_rename", review).is_err());
        let status = session.execute("session_status", json!({})).unwrap();
        assert_eq!(status["unchanged_field_rename_operation_id"], id);
        assert!(status["active_save_id"].is_null());
        assert!(
            session
                .execute(
                    "retry_unchanged_field_rename",
                    json!({"operation_id": id + 1}),
                )
                .is_err()
        );
        assert!(
            session
                .execute("open_database", json!({"path": path}))
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), different);
        assert!(!tablecraft_runtime::journal_path(&path).exists());
        if discard {
            let result = session
                .execute(
                    "discard_unchanged_field_rename",
                    json!({"operation_id": id}),
                )
                .unwrap();
            assert_eq!(result["native_write"], false);
            assert!(session.revision.is_none());
            assert_eq!(std::fs::read(&path).unwrap(), different);
        } else {
            std::fs::write(&path, &original).unwrap();
            let result = session
                .execute("retry_unchanged_field_rename", json!({"operation_id": id}))
                .unwrap();
            assert_eq!(result["verified_reopen"], true);
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        assert!(session.unchanged_field_rename.is_none());
        assert!(
            session
                .execute("retry_unchanged_field_rename", json!({"operation_id": id}),)
                .is_err()
        );
    }
}

#[test]
fn cancelled_unchanged_reopen_retains_the_actual_read_witness_for_retry() {
    let original = after_off::source(SourceCase::Number);
    let (_directory, path, mut session) = opened(&original);
    let review = review(&mut session, "Score");
    let id = review["review_id"].as_u64().unwrap();
    let (cancelled, setter) = cancel_after_dispatch(&mut session);
    assert!(
        session
            .execute_cancellable("apply_field_rename", review, cancelled)
            .is_err()
    );
    setter.join().unwrap();
    assert!(session.revision.is_none());
    assert!(session.unchanged_field_rename.is_some());
    let result = session
        .execute("retry_unchanged_field_rename", json!({"operation_id": id}))
        .unwrap();
    assert_eq!(result["verified_reopen"], true);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}

#[test]
fn case_only_field_rename_still_uses_real_save_and_independent_complete_after() {
    let original = after_off::source(SourceCase::Number);
    let mut expected = original.clone();
    // Independent literal TDEF/MR2 spans from the first-party raw fixture record.
    for offset in [28805, 45027] {
        assert_eq!(&original[offset..offset + 10], b"S\0c\0o\0r\0e\0");
        expected[offset..offset + 10].copy_from_slice(b"s\0c\0o\0r\0e\0");
    }
    let (_directory, path, mut session) = opened(&original);
    let review = review(&mut session, "score");
    assert_eq!(review["changed"], true);
    let id = save(&mut session, "apply_field_rename", review);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    acknowledge(&mut session, id, &path);
    assert!(session.unchanged_field_rename.is_none());
}

//! Explicit worker formats reach retrieval, criteria, assignments and VALUES.
use super::{assert_image, inspect, open, request, save_and_acknowledge, terminal};
use crate::{Command, Error, QueryExpressionFormats, Response, SaveOutcome, SaveWorker, Worker};
use std::path::PathBuf;
use tablecraft_access::editing::sql::{RecoverableError, RecoverableInsertError};
use tablecraft_expressions::EvalError;
use tablecraft_query::{InsertError, QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{deletes, field_changes, joined_update},
    database_properties,
};

fn opened(
    formats: QueryExpressionFormats,
) -> (tempfile::TempDir, PathBuf, Worker, u64, Vec<u8>, u32) {
    let source = joined_update::source();
    let before = database_properties::with_native17_properties(
        &source.bytes,
        &database_properties::name_auto_correct_disabled_with_query_mode(0),
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("formats.accdb");
    std::fs::write(&path, &before).unwrap();
    let worker = if formats == QueryExpressionFormats::default() {
        Worker::new(|| {})
    } else {
        Worker::with_query_formats(|| {}, formats)
    };
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("actual source with captured expression profiles");
    };
    (
        directory,
        path,
        worker,
        document_revision,
        before,
        source.target_definition,
    )
}

#[test]
fn explicit_formats_match_between_select_numeric_like_update_and_insert_values() {
    for (number, date, date_text, numeric_text, selected) in [
        ("en-us", "en-us", "12/31/2026", "1.25", 1),
        ("de-de", "de-de", "31.12.2026", "1,25", 1234),
        ("invariant", "iso", "2026-12-31", "1.25", 1),
    ] {
        let formats = QueryExpressionFormats::from_profile_names(Some(number), Some(date)).unwrap();
        let (_directory, path, worker, revision, before, definition) = opened(formats);
        let sql = format!(
            "SELECT CLng('1.234') AS Parsed, CStr(1.25) AS Spelling, Day(CDate('{date_text}')) AS DayNumber"
        );
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql,
            },
        )
        .unwrap() else {
            panic!("actual retrieval conversion context");
        };
        assert_eq!(rows[0][0].text, selected.to_string());
        assert_eq!(rows[0][1].text, numeric_text);
        assert_eq!(rows[0][2].text, "31");
        let pattern = if number == "de-de" { "0,17" } else { "0.17" };
        let sql = format!(
            "UPDATE Table1 SET Score=CLng('1.234')+Day(CDate('{date_text}'))-31 WHERE Score/100 LIKE '{pattern}'"
        );
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("actual format-dependent native UPDATE review");
        };
        assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let expected = field_changes::update_score(&before, definition, 0, Some(selected));
        save_and_acknowledge(capability, crate::journal_path(&path));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, open(&path, "")).unwrap()
        else {
            panic!("fresh native format-dependent update");
        };
        let sql =
            format!("INSERT INTO Table1(Score) VALUES(CLng('1.234')+Day(CDate('{date_text}'))-31)");
        let (target, _) = inspect(&worker, document_revision, &sql);
        let Response::QueryInsertReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryInsert {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("VALUES shares captured number syntax");
        };
        assert_eq!(review.inserted_rows, 1);
        let after = field_changes::append_score(&expected, definition, Some(selected));
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_insert(capability, crate::journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows.is_none() && receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()));
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &after);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh stored values after explicit-format actions");
        };
        assert_eq!(rows.len(), 3);
        assert_eq!(rows.last().unwrap()[1].text, selected.to_string());
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn late_bad_date_and_top_omitted_numeric_text_leave_all_original_records_unchanged() {
    let formats = QueryExpressionFormats::from_profile_names(Some("de-de"), Some("de-de")).unwrap();
    let (_directory, path, worker, revision, before, _) = opened(formats);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET Score=Day(CDate(IIf(Score IS NULL,'bad-date','31.12.2026')))",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(EvalError::TypeMismatch))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let (target, _) = inspect(
        &worker,
        revision,
        "INSERT INTO Table1(Score) SELECT TOP 1 CLng(IIf(Score IS NULL,'bad-number','1.234')) FROM Table1 ORDER BY ID",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryInsert {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryInsert(RecoverableInsertError::Plan(
            InsertError::Query(QueryError::Evaluation(EvalError::TypeMismatch))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Query {
            options: Default::default(),
            sql: "SELECT CLng('1.234') AS Value".into(),
        },
    )
    .unwrap() else {
        panic!("healthy retained worker context after refused actions");
    };
    assert_eq!(rows[0][0].text, "1234");
}

#[test]
fn missing_formats_remain_typed_and_regional_profiles_never_reinterpret_literal_envelopes() {
    let (_directory, path, worker, revision, before, _) = opened(QueryExpressionFormats::default());
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET Score=25 WHERE Score LIKE '1*'",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(EvalError::NumberFormatRequired))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    let formats = QueryExpressionFormats::from_profile_names(Some("de-de"), None).unwrap();
    let (_directory, path, worker, revision, before, _) = opened(formats);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET Score=Day(CDate('31.12.2026'))",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(EvalError::DateFormatRequired))
        )))
    ));
    let (target, _) = inspect(
        &worker,
        revision,
        "PARAMETERS v LONG; UPDATE Table1 SET Score=v WHERE ID=1",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: vec![crate::ParameterInput {
                    name: "v".into(),
                    kind: "int32".into(),
                    text: "1.234".into(),
                    is_null: false,
                }]
            }
        ),
        Err(Error::InvalidScalarInput)
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn delete_criteria_use_the_same_explicit_numeric_and_date_providers() {
    for (number, date, date_text) in [
        ("en-us", "en-us", "12/31/2026"),
        ("de-de", "de-de", "31.12.2026"),
    ] {
        let formats = QueryExpressionFormats::from_profile_names(Some(number), Some(date)).unwrap();
        let (_directory, path, worker, revision, before, definition) = opened(formats);
        let sql = format!(
            "DELETE FROM Table1 WHERE CStr(Score) LIKE '1*' AND Day(CDate('{date_text}'))=31"
        );
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryDeleteReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryDelete {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("actual provider-backed delete review");
        };
        assert_eq!((review.matched_rows, review.deleted_rows), (1, 1));
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let expected = deletes::after(&before, definition, &[0]);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_delete(capability, crate::journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()));
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        assert!(!crate::journal_path(&path).exists());
    }
}

//! Date intervals are evaluated completely before consuming a native review.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};

#[test]
fn explicit_date_intervals_updates_save_both_original_rows_and_reopen_full_raw_after() {
    let source = joined_update::source();
    let expected = field_changes::update_score(&source.bytes, source.target_definition, 0, Some(4));
    let expected = field_changes::update_score(&expected, source.target_definition, 1, Some(5));
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET Score=CLng(DatePart('m',DateAdd('m',ID,#2024-01-31#))+DateDiff('d',#2024-02-28#,#2024-03-01#))",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("complete native date interval review");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (2, 2));
    assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
    assert!(!crate::journal_path(&path).exists());

    let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
    assert_eq!(
        (receipt.matched_rows, receipt.changed_rows),
        (Some(2), Some(2))
    );
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("fresh native date interval rows");
    };
    assert_eq!(rows[0][1].text, "4");
    assert_eq!(rows[1][1].text, "5");
    assert_image(&std::fs::read(path).unwrap(), &expected);
}

#[test]
fn later_invalid_interval_and_date_domain_leave_no_native_prefix() {
    for (sql, expected) in [
        (
            "UPDATE Table1 SET Score=DatePart(IIf(ID=1,'d','invalid'),#2024-01-01#)",
            EvalError::InvalidDateInterval,
        ),
        (
            "UPDATE Table1 SET Score=Year(DateAdd('yyyy',1-ID,#0100-01-01#))",
            EvalError::Domain,
        ),
    ] {
        let source = joined_update::source();
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let (target, _) = inspect(&worker, revision, sql);
        assert!(
            matches!(
                request(
                    &worker,
                    Command::ReviewQueryUpdate {
                        target,
                        parameters: Vec::new()
                    }
                ),
                Err(Error::QueryUpdate(RecoverableError::Plan(
                    UpdateError::Query(QueryError::Evaluation(error))
                ))) if error == expected
            ),
            "{sql}"
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
        request(&worker, open(&path, "")).unwrap();
        assert_image(&std::fs::read(path).unwrap(), &source.bytes);
    }
}

#[test]
fn correlated_date_exists_and_in_save_one_complete_original_handle_selection() {
    for sql in [
        "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT DatePart('m',DateAdd('m',1,#2024-01-31#)) FROM Changes AS c WHERE c.Id=t.ID AND DateDiff('d',#2024-01-01#,#2024-01-02#)=1 AND Day(CDate(#2024-01-01#))=1 AND c.Score=5)",
        "UPDATE Table1 AS t SET Score=25 WHERE t.ID IN (SELECT CLng(DatePart('d',DateAdd('d',c.Id-1,#2024-01-01#))) FROM Changes AS c WHERE c.Id=t.ID AND c.Score=5)",
    ] {
        let source = joined_update::source();
        let expected =
            field_changes::update_score(&source.bytes, source.target_definition, 0, Some(25));
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let (target, _) = inspect(&worker, revision, sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap_or_else(|error| panic!("correlated date review {sql}: {error:?}")) else {
            panic!("complete correlated date review");
        };
        assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());

        let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(1), Some(1))
        );
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn late_correlated_date_projection_failure_and_closed_nested_calls_never_publish() {
    for (sql, expected) in [
        (
            "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT DatePart(IIf(t.ID=1,'d','invalid'),#2024-01-01#) FROM Changes AS c WHERE c.Id=t.ID)",
            QueryError::Evaluation(EvalError::InvalidDateInterval),
        ),
        (
            "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT CVDate(1) FROM Changes AS c WHERE c.Id=t.ID)",
            QueryError::Unsupported,
        ),
        (
            "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT CDate(c.Score LIKE '5') FROM Changes AS c WHERE c.Id=t.ID)",
            QueryError::Unsupported,
        ),
        (
            "UPDATE Table1 AS t SET Score=25 WHERE EXISTS (SELECT DatePart('d',Array(1)) FROM Changes AS c WHERE c.Id=t.ID)",
            QueryError::Unsupported,
        ),
    ] {
        let source = joined_update::source();
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let (target, _) = inspect(&worker, revision, sql);
        let result = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        );
        assert!(
            matches!(result, Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) if actual == expected),
            "{sql}"
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
}

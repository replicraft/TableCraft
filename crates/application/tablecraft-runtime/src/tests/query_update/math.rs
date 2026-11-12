//! Pure scalar math is evaluated completely before consuming a native review.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};

#[test]
fn explicit_integral_math_updates_save_both_original_rows_and_reopen_full_raw_after() {
    let source = joined_update::source();
    let expected = field_changes::update_score(&source.bytes, source.target_definition, 0, Some(2));
    let expected = field_changes::update_score(&expected, source.target_definition, 1, Some(4));
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET Score=CLng(Sqr(ID*ID*4))",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("complete native math review");
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
        panic!("fresh native math rows");
    };
    assert_eq!(rows[0][1].text, "2");
    assert_eq!(rows[1][1].text, "4");
    assert_image(&std::fs::read(path).unwrap(), &expected);
}

#[test]
fn later_math_domain_and_conversion_overflow_leave_no_native_prefix() {
    for sql in [
        "UPDATE Table1 SET Score=CLng(Sqr(2-ID*2))",
        "UPDATE Table1 SET Score=CLng(Exp(ID*11))",
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
                    UpdateError::Query(QueryError::Evaluation(_))
                )))
            ),
            "{sql}"
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
        request(&worker, open(&path, "")).unwrap();
        assert_image(&std::fs::read(path).unwrap(), &source.bytes);
    }
}

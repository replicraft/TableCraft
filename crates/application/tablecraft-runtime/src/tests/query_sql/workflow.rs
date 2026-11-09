//! Distinct full-SQL capability, save ownership and unchanged shared reopening.
use super::{inspect, opened, request, review};
use crate::{
    Command, Error, QuerySqlEligibility, Response, SaveOutcome, SaveQueryChange, SaveStartFailure,
    SaveWorker, SavedQueryTarget,
};
use tablecraft_testkit::fixtures::access::{
    query_expression_edits as raw, query_sql_expression_edits as sql_raw,
};

#[test]
fn two_expression_save_consumes_distinct_capability_and_reopens_full_image() {
    let before = raw::two_projections();
    let after = raw::replace_at(&raw::replace_at(&before, 3, "1000"), 4, "7000");
    let (directory, path, worker, target) = opened(&before);
    let original = inspect(&worker, &target).sql.unwrap();
    let Response::QuerySqlReviewed {
        review: facts,
        capability,
    } = review(
        &worker,
        &target,
        &original,
        "SELECT 1000 AS Answer,7000 AS [Second];",
    )
    else {
        panic!("expected distinct full SQL capability");
    };
    let Response::QuerySqlReviewed {
        capability: second, ..
    } = review(
        &worker,
        &target,
        &original,
        "SELECT 1001 AS Answer,7001 AS [Second];",
    )
    else {
        panic!("expected independent lease-free review");
    };
    assert_eq!(facts.target, target);
    assert!(facts.changed);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_sql(capability, directory.path().join("query.journal"))
        .unwrap();
    let rejected = saves
        .start_query_sql(second, directory.path().join("other.journal"))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::Busy);
    assert!(rejected.into_reviewed().is_some());
    let report = crate::tests::terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected exact full SQL save");
    };
    assert_eq!(receipt.query_change, Some(SaveQueryChange::ExpressionSaved));
    assert_eq!(receipt.schema_change, None);
    assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(saves.acknowledge(id));
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(&worker, Command::OpenQueryExpressionSaved { witness }).unwrap()
    else {
        panic!("same complete-image query witness must reopen");
    };
    let query = queries.iter().find(|query| query.name == "Saved").unwrap();
    let fresh = SavedQueryTarget {
        document_revision,
        object_id: query.object_id,
        name: query.name.clone(),
    };
    let Response::Data { rows, .. } = request(
        &worker,
        Command::RunSavedQuery {
            target: fresh,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("explicit Run remains separate from Save");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1000");
    assert_eq!(rows[0][1].text, "7000");
}

#[test]
fn scalar_expression_syntax_is_reviewed_inertly_and_low_level_literal_stays_frozen() {
    let before = raw::source();
    let (_directory, path, worker, target) = opened(&before);
    for proposed in [
        "SELECT 40+3 AS Answer;",
        "SELECT NeverExecuteMe() AS Answer;",
        "SELECT -1 AS Answer;",
        "SELECT 043 AS Answer;",
        "SELECT (43) AS Answer;",
        "SELECT +43 AS Answer;",
        "SELECT 43.0 AS Answer;",
        "SELECT 43e0 AS Answer;",
        "SELECT &H2B AS Answer;",
        "SELECT 2147483648 AS Answer;",
    ] {
        let Response::QuerySqlReviewed { review: facts, .. } =
            review(&worker, &target, "SELECT 42 AS Answer;", proposed)
        else {
            panic!("expected inert supported expression review");
        };
        assert!(facts.changed);
        assert_eq!(facts.proposed_sql, proposed);
    }
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryLiteralExpression {
                target,
                expected: "42".into(),
                replacement: "1000".into(),
            }
        ),
        Err(Error::QueryExpression(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn where_review_has_separate_native_capability_and_no_user_data_execution() {
    let before = sql_raw::local_where();
    let (_directory, path, worker, target) = opened(&before);
    let draft = inspect(&worker, &target);
    assert_eq!(
        draft.eligibility,
        QuerySqlEligibility::QualifiedExpressionSelect
    );
    let Response::QuerySqlReviewed {
        review: facts,
        capability,
    } = review(
        &worker,
        &target,
        draft.sql.as_deref().unwrap(),
        "SELECT [p].[Id] AS Answer,99 AS [Second] FROM MSysObjects AS p WHERE [p].[Id]>=1000;",
    )
    else {
        panic!("expected projection and WHERE review");
    };
    assert!(facts.changed);
    assert_eq!(facts.target, target);
    assert_eq!(capability.reopen_witness().native_id(), facts.native_id);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn new_sql_capability_keeps_document_invalidation_and_complete_reopen_guard() {
    let before = raw::two_projections();
    let (directory, path, worker, target) = opened(&before);
    let original = "SELECT 42 AS Answer,99 AS [Second];";
    let proposed = "SELECT 1000 AS Answer,7000 AS [Second];";
    let Response::QuerySqlReviewed { capability, .. } =
        review(&worker, &target, original, proposed)
    else {
        panic!("expected full SQL review");
    };
    let witness = capability.reopen_witness();
    let mut changed = witness.expected_bytes().to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenQueryExpressionSaved { witness }),
        Err(Error::QueryExpressionSourceChanged)
    ));
    std::fs::write(&path, &before).unwrap();
    request(&worker, crate::tests::open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let rejected = saves
        .start_query_sql(capability, directory.path().join("stale.journal"))
        .unwrap_err();
    assert_eq!(rejected.kind, SaveStartFailure::StaleDocument);
    assert!(rejected.into_reviewed().is_some());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

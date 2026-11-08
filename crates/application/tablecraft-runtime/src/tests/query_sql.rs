//! Full SQL review uses the native target and never writes or executes a draft.
use super::{request, saved::opened};
use crate::{
    Command, Error, QuerySqlDraft, QuerySqlEditError, QuerySqlEligibility, Response,
    SavedQueryTarget, Worker,
};
use tablecraft_testkit::fixtures::access::saved_queries::{
    Fragment, QUERY_ID, database_with_query_flags, fitting_select::storage_pair,
};

const FIXTURE_PAGE_BYTES: usize = 4096;
const USER_DATA_PAGE: usize = 12;

fn inspect(worker: &Worker, target: &SavedQueryTarget) -> QuerySqlDraft {
    let Response::QuerySqlDraft { draft } = request(
        worker,
        Command::InspectQuerySqlDraft {
            target: target.clone(),
        },
    )
    .unwrap() else {
        panic!("expected inert source-bound SQL draft");
    };
    assert_eq!(draft.target, *target);
    draft
}

fn review(worker: &Worker, target: &SavedQueryTarget, original: &str, proposed: &str) -> Response {
    request(
        worker,
        Command::ReviewQuerySql {
            target: target.clone(),
            original_sql: original.into(),
            proposed_sql: proposed.into(),
        },
    )
    .unwrap()
}

#[test]
fn inert_draft_and_formatting_only_review_preserve_every_source_byte() {
    let [before, _] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    let draft = inspect(&worker, &target);
    assert_eq!(
        draft.eligibility,
        QuerySqlEligibility::QualifiedConstantSelect
    );
    let original = draft.sql.unwrap();
    assert_eq!(original, "SELECT 42 AS [Answer];");
    let formatted = "  select ALL 42 as [Answer] ; ";
    let Response::QuerySqlReviewed { review, capability } =
        review(&worker, &target, &original, formatted)
    else {
        panic!("expected formatting-only owning review");
    };
    assert!(!review.expression.changed);
    assert_eq!(review.expression.target, target);
    assert_eq!(review.original_sql, original);
    assert_eq!(review.proposed_sql, formatted);
    assert_eq!(capability.source_path(), path);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn changed_literal_review_retains_complete_submitted_sql_and_exact_target() {
    let [before, _] = storage_pair();
    let (_directory, path, worker, target) = opened(&before);
    let Response::QuerySqlReviewed { review, capability } = review(
        &worker,
        &target,
        "select 42 as Answer;",
        "SELECT 43 AS [Answer];",
    ) else {
        panic!("expected qualified literal change");
    };
    assert!(review.expression.changed);
    assert_eq!(review.expression.target, target);
    assert_eq!(review.expression.expected, "42");
    assert_eq!(review.expression.replacement, "43");
    assert_eq!(review.original_sql, "select 42 as Answer;");
    assert_eq!(review.proposed_sql, "SELECT 43 AS [Answer];");
    assert_eq!(capability.reopen_witness().query_name(), "Saved");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn changed_alias_sources_clauses_parameters_and_expressions_cannot_gain_authority() {
    let [before, _] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    for proposed in [
        "SELECT 43 AS Other;",
        "SELECT 43 AS Answer FROM NeverOpenMe;",
        "SELECT 43 AS Answer FROM Items IN 'NeverOpenMe.accdb';",
        "SELECT TOP 1 43 AS Answer;",
        "SELECT DISTINCT 43 AS Answer;",
        "SELECT 43 AS Answer WHERE True;",
        "SELECT 43 AS Answer ORDER BY 1;",
        "PARAMETERS p LONG; SELECT 43 AS Answer;",
        "SELECT 43 AS Answer WITH OWNERACCESS OPTION;",
        "SELECT 43 AS Answer UNION SELECT 44 AS Answer;",
        "SELECT 40 + 3 AS Answer;",
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
        assert!(
            matches!(
                request(
                    &worker,
                    Command::ReviewQuerySql {
                        target: target.clone(),
                        original_sql: "SELECT 42 AS Answer;".into(),
                        proposed_sql: proposed.into(),
                    },
                ),
                Err(Error::QuerySqlEdit(QuerySqlEditError::Unsupported))
            ),
            "unexpected qualification for {proposed}"
        );
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn original_definition_width_and_stale_target_checks_remain_independent() {
    let [before, _] = storage_pair();
    let (_directory, path, worker, target) = opened(&before);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQuerySql {
                target: target.clone(),
                original_sql: "SELECT 41 AS Answer;".into(),
                proposed_sql: "SELECT 43 AS Answer;".into(),
            }
        ),
        Err(Error::QuerySqlEdit(QuerySqlEditError::OriginalChanged))
    ));
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQuerySql {
                target: target.clone(),
                original_sql: "SELECT 42 AS Answer;".into(),
                proposed_sql: "SELECT 100 AS Answer;".into(),
            }
        ),
        Err(Error::QueryExpression(_))
    ));
    let mut wrong_identity = target.clone();
    wrong_identity.object_id ^= 1;
    let mut wrong_revision = target.clone();
    wrong_revision.document_revision += 1;
    for stale in [wrong_identity, wrong_revision] {
        assert!(matches!(
            request(
                &worker,
                Command::InspectQuerySqlDraft {
                    target: stale.clone()
                }
            ),
            Err(Error::StaleSavedQuery)
        ));
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQuerySql {
                    target: stale,
                    original_sql: "SELECT 42 AS Answer;".into(),
                    proposed_sql: "SELECT 43 AS Answer;".into(),
                }
            ),
            Err(Error::StaleSavedQuery)
        ));
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn nonprofile_reconstructed_queries_have_no_editable_sql_or_capability() {
    let [mut before, _] = storage_pair();
    let alias: Vec<u8> = "Answer".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let matches: Vec<usize> = before
        .windows(alias.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == alias).then_some(offset))
        .collect();
    assert_eq!(matches.len(), 1);
    // Independent raw same-width alias change; never alter an original fixture.
    before[matches[0] + 10] = b's';
    let (_directory, path, worker, target) = opened(&before);
    let draft = inspect(&worker, &target);
    assert_eq!(draft.eligibility, QuerySqlEligibility::Unavailable);
    assert_eq!(draft.sql.as_deref(), Some("SELECT 42 AS [Answes];"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn readable_table_query_sql_is_visible_without_scanning_rows_or_qualifying_writes() {
    let mut before = database_with_query_flags(
        &[
            Fragment {
                object: Some(QUERY_ID),
                attribute: Some(5),
                order: Some(&[0, 0, 0, 1]),
                name1: Some("Items"),
                ..Default::default()
            },
            Fragment {
                object: Some(QUERY_ID),
                attribute: Some(6),
                order: Some(&[0, 0, 0, 1]),
                expression: Some("Id"),
                ..Default::default()
            },
        ],
        0,
    );
    // This independently authored fixture's user DATA page is unreadable. The
    // SQL view must reconstruct only the stored query, without running it.
    before[USER_DATA_PAGE * FIXTURE_PAGE_BYTES] = 0;
    let (directory, path, worker, target) = opened(&before);
    let draft = inspect(&worker, &target);
    let original = draft.sql.unwrap();
    assert_eq!(original, "SELECT [Id] FROM [Items];");
    assert_eq!(draft.eligibility, QuerySqlEligibility::Unavailable);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQuerySql {
                target,
                original_sql: original,
                proposed_sql: "SELECT 43 AS Answer;".into(),
            }
        ),
        Err(Error::QuerySqlEdit(QuerySqlEditError::Unsupported))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn original_sql_and_native_expression_spelling_cannot_be_silently_normalized() {
    let [before, _] = storage_pair();
    let (_directory, path, worker, target) = opened(&before);
    for original in [
        "SELECT 042 AS Answer;",
        "SELECT (42) AS Answer;",
        "SELECT +42 AS Answer;",
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQuerySql {
                    target: target.clone(),
                    original_sql: original.into(),
                    proposed_sql: "SELECT 43 AS Answer;".into(),
                }
            ),
            Err(Error::QuerySqlEdit(QuerySqlEditError::Unsupported))
        ));
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);

    let mut noncanonical = before;
    let expression = [b'4', 0, b'2', 0];
    let matches: Vec<usize> = noncanonical
        .windows(expression.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == expression).then_some(offset))
        .collect();
    assert_eq!(matches.len(), 1);
    noncanonical[matches[0]] = b'0';
    noncanonical[matches[0] + 2] = b'4';
    let (_directory, path, worker, target) = opened(&noncanonical);
    let draft = inspect(&worker, &target);
    assert_eq!(draft.eligibility, QuerySqlEligibility::Unavailable);
    assert!(draft.sql.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), noncanonical);
}

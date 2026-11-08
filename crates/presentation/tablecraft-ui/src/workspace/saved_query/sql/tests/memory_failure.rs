use super::inspected;
use crate::{SavedQuerySqlError, workspace::objects::MAX_RETAINED_BYTES};

#[test]
fn oversized_failure_report_releases_only_the_matching_review_without_losing_text() {
    let (mut state, _) = inspected();
    let proposed = " SELECT 43 AS [Answer]; -- keep my draft\n";
    assert!(state.set_saved_query_sql_text(proposed.into()));
    let revision = state.saved_query_sql().unwrap().2;
    let request = state.begin_saved_query_sql_review(20).unwrap();

    let report = String::with_capacity(MAX_RETAINED_BYTES);
    assert!(!state.fail_saved_query_sql(19, &request, report));
    assert!(state.saved_query_sql_pending());
    assert!(state.fail_saved_query_sql(20, &request, String::with_capacity(MAX_RETAINED_BYTES),));

    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::MemoryLimit)
    );
    assert!(!state.saved_query_sql_pending());
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 42 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().1, proposed);
    assert_eq!(state.saved_query_sql().unwrap().2, revision);
    assert!(state.saved_query_sql_dirty());
    assert!(state.can_review_saved_query_sql());
}

#[test]
fn accepted_review_can_return_to_its_dirty_draft_when_the_report_cannot_fit() {
    let (mut state, _) = inspected();
    let proposed = "SELECT 43 AS Answer";
    assert!(state.set_saved_query_sql_text(proposed.into()));
    let revision = state.saved_query_sql().unwrap().2;
    let request = state.begin_saved_query_sql_review(20).unwrap();
    assert!(state.set_saved_query_sql_review(20, &request, 70));
    assert!(state.release_saved_query_sql_review(
        20,
        &request,
        70,
        Some(String::with_capacity(MAX_RETAINED_BYTES)),
    ));

    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::MemoryLimit)
    );
    assert!(!state.saved_query_sql_pending());
    assert_eq!(state.saved_query_sql().unwrap().1, proposed);
    assert_eq!(state.saved_query_sql().unwrap().2, revision);
    assert!(state.can_review_saved_query_sql());
}

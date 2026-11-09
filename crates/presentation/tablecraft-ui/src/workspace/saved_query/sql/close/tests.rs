use super::*;
use crate::workspace::{ObjectKind, ObjectTarget, SavedQueryTarget};
use crate::{Database, SavedQuery};

fn dirty() -> (State, ObjectTab) {
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 90,
        name: "Query 空".into(),
    };
    let mut state = State::default();
    state.saved_query_sql_edit_available = true;
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![SavedQuery {
            object_id: 90,
            name: target.name.clone(),
            system: false,
        }],
    });
    state.begin_saved_query_inspection(target.clone(), 1);
    assert!(state.set_saved_query_details(1, target.clone(), vec![]));
    let request = state.begin_saved_query_sql_inspection(target, 2).unwrap();
    assert!(state.set_saved_query_sql_inspection(
        2,
        &request,
        Some("SELECT 42 AS Answer".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect
    ));
    assert!(state.set_saved_query_sql_text(" select 43 as [Answer];\n".into()));
    let tab = state.active_object().unwrap();
    (state, tab)
}

#[test]
fn dirty_tab_close_opens_confirmation_and_cancel_preserves_exact_text() {
    let (mut state, tab) = dirty();
    assert!(!state.close_object(&tab));
    assert!(state.saved_query_sql_close_open());
    assert!(!state.can_edit_saved_query_sql());
    let result = state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Cancel)
        .unwrap();
    assert_eq!(result.decision, SavedQuerySqlCloseDecision::Cancel);
    assert_eq!(result.tab, tab);
    assert!(result.request.is_none());
    assert!(!state.saved_query_sql_close_open());
    assert!(state.has_modified_saved_query_sql());
    assert_eq!(
        state.saved_query_sql().unwrap().1,
        " select 43 as [Answer];\n"
    );
}

#[test]
fn save_returns_original_and_proposed_without_clearing_dirty_state() {
    let (mut state, tab) = dirty();
    let revision = state.saved_query_sql_revision_for(&tab).unwrap();
    assert!(state.request_saved_query_sql_close(&tab));
    assert!(state.can_save_saved_query_sql_close());
    let result = state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
        .unwrap();
    assert_eq!(result.decision, SavedQuerySqlCloseDecision::Save);
    assert_eq!(result.draft_revision, revision);
    assert_eq!(result.tab, tab);
    let request = result.request.unwrap();
    assert_eq!(request.original_sql.as_deref(), Some("SELECT 42 AS Answer"));
    assert_eq!(request.proposed_sql, " select 43 as [Answer];\n");
    assert!(state.saved_query_sql_request_current(&request));
    assert!(state.has_modified_saved_query_sql());
    assert!(!state.close_object_confirmed(&tab));
}

#[test]
fn explicit_discard_clears_only_the_bound_query_then_close_can_finish() {
    let (mut state, tab) = dirty();
    assert!(state.request_saved_query_sql_close(&tab));
    let result = state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
        .unwrap();
    assert_eq!(result.decision, SavedQuerySqlCloseDecision::Discard);
    assert!(result.request.is_none());
    assert!(!state.has_modified_saved_query_sql());
    assert!(state.close_object_confirmed(&tab));
}

#[test]
fn stale_save_and_discard_leave_newer_text_and_confirmation_intact() {
    let (mut state, tab) = dirty();
    assert!(state.request_saved_query_sql_close(&tab));
    assert!(
        state
            .saved_query
            .sql
            .as_mut()
            .unwrap()
            .replace_text("SELECT 44 AS Answer".into())
    );
    assert!(!state.can_save_saved_query_sql_close());
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
            .is_none()
    );
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
            .is_none()
    );
    assert!(state.saved_query_sql_close_open());
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 44 AS Answer");
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Cancel)
            .is_some()
    );
    assert!(state.has_modified_saved_query_sql());
    assert!(state.request_saved_query_sql_close(&tab));
    state.database.as_mut().unwrap().document_revision += 1;
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
            .is_none()
    );
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
            .is_none()
    );
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 44 AS Answer");
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Cancel)
            .is_some()
    );
}

#[test]
fn parked_query_save_activates_its_exact_owner_and_retains_draft() {
    let (mut state, tab) = dirty();
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    assert_ne!(state.active_object(), Some(tab.clone()));
    assert!(state.request_saved_query_sql_close(&tab));
    let result = state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
        .unwrap();
    assert_eq!(state.active_object(), Some(tab));
    assert!(state.saved_query_sql_request_current(&result.request.unwrap()));
    assert!(state.has_modified_saved_query_sql());
}

#[test]
fn readonly_host_eligibility_disables_save_but_allows_explicit_discard() {
    let (mut state, tab) = dirty();
    state.saved_query.sql.as_mut().unwrap().eligibility = SavedQuerySqlEligibility::Unavailable;
    assert!(state.request_saved_query_sql_close(&tab));
    assert!(!state.can_save_saved_query_sql_close());
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
            .is_none()
    );
    assert!(state.has_modified_saved_query_sql());
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
            .is_some()
    );
    assert!(!state.has_modified_saved_query_sql());
}

#[test]
fn direct_and_bulk_close_cannot_bypass_confirmation_and_memory_is_counted() {
    let (mut state, tab) = dirty();
    assert!(!state.close_object_confirmed(&tab));
    assert!(!state.close_all_objects());
    let before = state.object_bytes().unwrap();
    assert!(state.request_saved_query_sql_close_all());
    assert!(state.object_bytes().unwrap() > before);
    assert!(!state.close_all_objects());
    assert!(state.has_modified_saved_query_sql());
    assert!(
        state
            .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
            .is_some()
    );
    assert!(state.close_all_objects());
}

use super::*;
use crate::workspace::{ObjectKind, ObjectTarget, SavedQueryTarget, State};
use crate::{Database, SavedQuery};

mod memory_failure;
mod mutation_guards;

fn state() -> (State, SavedQueryTarget) {
    let target = SavedQueryTarget {
        document_revision: 7,
        object_id: 90,
        name: "Saved 空".into(),
    };
    let mut state = State {
        saved_query_sql_edit_available: true,
        ..State::default()
    };
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
    (state, target)
}

fn inspected() -> (State, SavedQueryTarget) {
    let (mut state, target) = state();
    let request = state
        .begin_saved_query_sql_inspection(target.clone(), 10)
        .unwrap();
    assert!(state.set_saved_query_sql_inspection(
        10,
        &request,
        Some("SELECT 42 AS Answer".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect
    ));
    (state, target)
}

fn copy(request: &SavedQuerySqlRequest) -> SavedQuerySqlRequest {
    SavedQuerySqlRequest {
        target: request.target.clone(),
        draft_revision: request.draft_revision,
        original_sql: request.original_sql.clone(),
        proposed_sql: request.proposed_sql.clone(),
    }
}

#[test]
fn host_inspection_is_readonly_fact_and_exact_object_identity_is_required() {
    let (mut state, target) = state();
    let request = state.begin_saved_query_sql_inspection(target, 10).unwrap();
    for bad in 0..4 {
        let mut stale = copy(&request);
        match bad {
            0 => stale.target.document_revision += 1,
            1 => stale.target.object_id += 1,
            2 => stale.target.name.push('x'),
            _ => stale.draft_revision += 1,
        }
        assert!(!state.set_saved_query_sql_inspection(
            10,
            &stale,
            Some("SQL".into()),
            SavedQuerySqlEligibility::QualifiedExpressionSelect
        ));
    }
    assert!(state.set_saved_query_sql_inspection(
        10,
        &request,
        None,
        SavedQuerySqlEligibility::Unavailable
    ));
    assert!(!state.set_saved_query_sql_text("invented".into()));
    assert!(!state.saved_query_sql_dirty());
    assert!(!state.can_review_saved_query_sql());
}

#[test]
fn exact_user_text_is_dirty_and_edit_invalidates_inspection_reply() {
    let (mut state, target) = inspected();
    assert!(!state.saved_query_sql_dirty());
    let pending = state.begin_saved_query_sql_inspection(target, 11).unwrap();
    let user = " select  43 as [Answer];\n";
    assert!(state.set_saved_query_sql_text(user.into()));
    assert!(state.saved_query_sql_dirty());
    assert!(!state.set_saved_query_sql_inspection(
        11,
        &pending,
        Some("SELECT 42 AS Answer".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect
    ));
    let (original, proposed, revision, _) = state.saved_query_sql().unwrap();
    assert_eq!(original, "SELECT 42 AS Answer");
    assert_eq!(proposed, user);
    assert!(revision > pending.draft_revision);
}

#[test]
fn changed_original_on_reinspection_keeps_dirty_text_and_disables_eligibility() {
    let (mut state, target) = inspected();
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let pending = state.begin_saved_query_sql_inspection(target, 11).unwrap();
    assert!(!state.set_saved_query_sql_inspection(
        11,
        &pending,
        Some("SELECT 99 AS Answer".into()),
        SavedQuerySqlEligibility::QualifiedExpressionSelect
    ));
    assert_eq!(
        state.saved_query_sql_error(),
        Some(&SavedQuerySqlError::SourceChanged)
    );
    assert_eq!(state.saved_query_sql().unwrap().0, "SELECT 42 AS Answer");
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
    assert!(state.saved_query_sql_dirty());
    assert!(!state.can_edit_saved_query_sql());
}

#[test]
fn failed_and_stale_reviews_keep_text_and_cannot_match_a_newer_aba_revision() {
    let (mut state, _) = inspected();
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let first = state.begin_saved_query_sql_review(20).unwrap();
    assert!(state.fail_saved_query_sql(20, &first, "native source refused".into()));
    assert!(state.saved_query_sql_dirty());
    let second = state.begin_saved_query_sql_review(21).unwrap();
    assert!(!state.set_saved_query_sql_review(20, &first, 70));
    assert!(state.set_saved_query_sql_review(21, &second, 71));
    assert!(state.matches_saved_query_sql_review(21, &second, 71));
    assert!(!state.matches_saved_query_sql_review(21, &second, 70));
    assert!(state.set_saved_query_sql_text("SELECT 44 AS Answer".into()));
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    assert!(!state.matches_saved_query_sql_review(21, &second, 71));
    assert!(!state.set_saved_query_sql_review(21, &second, 71));
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
}

#[test]
fn parked_saved_session_retains_draft_dirty_revision_and_pending_copies() {
    let (mut state, _) = inspected();
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let tab = state.active_object().unwrap();
    let revision = state.saved_query_sql_revision_for(&tab).unwrap();
    let before = state.object_bytes().unwrap();
    let request = state.begin_saved_query_sql_review(20).unwrap();
    assert!(state.object_bytes().unwrap() > before);
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    assert!(state.has_modified_saved_query_sql());
    assert_eq!(state.saved_query_sql_revision_for(&tab), Some(revision));
    assert!(state.activate_object(&tab));
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
    assert!(state.saved_query_sql_dirty());
    assert!(state.can_edit_saved_query_sql());
    assert!(state.open_initial_table_draft());
    assert!(state.activate_object(&tab));
    assert!(state.has_unsaved_table_drafts());
    assert!(!state.can_edit_saved_query_sql());
    assert!(!state.can_review_saved_query_sql());
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
}

#[test]
fn explicit_discard_is_bound_to_actual_object_and_text_revision() {
    let (mut state, _) = inspected();
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let tab = state.active_object().unwrap();
    let revision = state.saved_query_sql_revision_for(&tab).unwrap();
    assert!(state.set_saved_query_sql_text("SELECT 44 AS Answer".into()));
    assert!(!state.discard_saved_query_sql(&tab, revision));
    assert!(state.saved_query_sql_dirty());
    let revision = state.saved_query_sql_revision_for(&tab).unwrap();
    assert!(!state.close_object(&tab));
    assert!(state.saved_query_sql_close_open());
    assert!(!state.discard_saved_query_sql(&tab, revision));
    let choice = state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
        .unwrap();
    assert_eq!(choice.tab, tab);
    assert_eq!(choice.draft_revision, revision);
    assert_eq!(choice.decision, SavedQuerySqlCloseDecision::Discard);
    assert!(!state.has_modified_saved_query_sql());
    assert!(state.close_object(&tab));
}

#[test]
fn memory_counts_spare_capacities_in_all_owned_strings_and_pending_snapshot() {
    let (mut state, target) = inspected();
    let draft = state.saved_query.sql.as_mut().unwrap();
    draft.target.name.reserve(512);
    draft.original.as_mut().unwrap().reserve(1024);
    draft.proposed.reserve(2048);
    let base = draft.retained_bytes().unwrap();
    let expected_minimum = draft.target.name.capacity()
        + draft.original.as_ref().unwrap().capacity()
        + draft.proposed.capacity();
    assert!(base >= expected_minimum);
    let request = state.begin_saved_query_sql_inspection(target, 30).unwrap();
    let draft = state.saved_query.sql.as_ref().unwrap();
    assert!(
        draft.retained_bytes().unwrap()
            >= base
                + request.target.name.len()
                + request.original_sql.as_ref().unwrap().len()
                + request.proposed_sql.len()
    );
}

#[test]
fn ordinary_parameter_reinspection_keeps_sql_text_and_dirty_revision() {
    let (mut state, target) = inspected();
    assert!(state.set_saved_query_sql_text("SELECT 43 AS Answer".into()));
    let tab = state.active_object().unwrap();
    let revision = state.saved_query_sql_revision_for(&tab);
    state.begin_saved_query_inspection(target.clone(), 31);
    assert!(state.set_saved_query_details(31, target, vec![]));
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 43 AS Answer");
    assert_eq!(state.saved_query_sql_revision_for(&tab), revision);
    assert!(!state.saved_query_run_ready());
}

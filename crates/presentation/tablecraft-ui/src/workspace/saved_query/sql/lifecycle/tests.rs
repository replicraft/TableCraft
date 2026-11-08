use super::*;
use crate::workspace::{ObjectKind, ObjectTarget, SavedQuerySqlEligibility, SavedQueryTarget};
use crate::{Database, SavedQuery};

fn reviewed() -> (State, SavedQuerySqlRequest) {
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
        SavedQuerySqlEligibility::QualifiedConstantSelect
    ));
    assert!(state.set_saved_query_sql_text(" select 43 as [Answer];\n".into()));
    let request = state.begin_saved_query_sql_review(3).unwrap();
    assert!(state.set_saved_query_sql_review(3, &request, 70));
    (state, request)
}

#[test]
fn release_and_fail_preserve_exact_dirty_text_baseline_and_revision() {
    for message in [None, Some("Original image retained".into())] {
        let (mut state, request) = reviewed();
        assert!(state.release_saved_query_sql_review(3, &request, 70, message));
        assert!(state.saved_query_sql_dirty());
        let (original, proposed, revision, _) = state.saved_query_sql().unwrap();
        assert_eq!(original, request.original_sql.as_deref().unwrap());
        assert_eq!(proposed, request.proposed_sql);
        assert_eq!(revision, request.draft_revision);
        assert!(!state.saved_query_sql_pending());
        assert!(state.can_review_saved_query_sql());
        assert!(!state.resolve_saved_query_sql_review(3, &request, 70));
    }
}

#[test]
fn only_exact_accepted_review_resolves_to_the_submitted_text_baseline() {
    let (mut state, request) = reviewed();
    assert!(!state.resolve_saved_query_sql_review(4, &request, 70));
    assert!(!state.resolve_saved_query_sql_review(3, &request, 71));
    assert!(state.saved_query_sql_dirty());
    assert!(state.resolve_saved_query_sql_review(3, &request, 70));
    let (original, proposed, revision, _) = state.saved_query_sql().unwrap();
    assert_eq!(original, request.proposed_sql);
    assert_eq!(proposed, request.proposed_sql);
    assert_eq!(revision, request.draft_revision + 1);
    assert!(!state.saved_query_sql_dirty());
    assert!(!state.saved_query_sql_pending());
    assert!(!state.resolve_saved_query_sql_review(3, &request, 70));
}

#[test]
fn stale_target_changed_text_and_unaccepted_phase_cannot_resolve_or_release() {
    let (mut state, request) = reviewed();
    assert!(state.set_saved_query_sql_text("SELECT 44 AS Answer".into()));
    assert!(!state.resolve_saved_query_sql_review(3, &request, 70));
    assert!(!state.release_saved_query_sql_review(3, &request, 70, None));
    let unaccepted = state.begin_saved_query_sql_review(4).unwrap();
    assert!(!state.resolve_saved_query_sql_review(4, &unaccepted, 71));
    assert!(!state.release_saved_query_sql_review(4, &unaccepted, 71, None));
    assert!(state.set_saved_query_sql_review(4, &unaccepted, 71));
    state.database.as_mut().unwrap().document_revision += 1;
    assert!(!state.resolve_saved_query_sql_review(4, &unaccepted, 71));
    assert!(!state.release_saved_query_sql_review(4, &unaccepted, 71, None));
    assert_eq!(state.saved_query_sql().unwrap().1, "SELECT 44 AS Answer");
    assert!(state.has_modified_saved_query_sql());
}

#[test]
fn terminal_callbacks_find_parked_owner_and_refresh_retained_memory() {
    let (mut state, request) = reviewed();
    let tab = state.active_object().unwrap();
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    let before = state.object_bytes().unwrap();
    let mut message = String::with_capacity(1024);
    message.push_str("Original source retained");
    assert!(state.release_saved_query_sql_review(3, &request, 70, Some(message)));
    assert!(state.has_modified_saved_query_sql());
    assert!(state.object_bytes().unwrap() > before);
    assert!(state.activate_object(&tab));
    let request = state.begin_saved_query_sql_review(4).unwrap();
    assert!(state.set_saved_query_sql_review(4, &request, 71));
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Sql,
        })
        .unwrap();
    assert!(state.resolve_saved_query_sql_review(4, &request, 71));
    assert!(!state.has_modified_saved_query_sql());
    assert!(state.activate_object(&tab));
    assert_eq!(state.saved_query_sql().unwrap().1, request.proposed_sql);
}

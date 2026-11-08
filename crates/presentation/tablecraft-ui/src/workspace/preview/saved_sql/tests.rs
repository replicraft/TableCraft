use super::*;
use crate::Action;
use crate::workspace::active_save::tests::inspected_state;
use crate::workspace::preview::Scene;

fn dirty_preview() -> Preview {
    let (mut state, _) = inspected_state(true);
    assert!(state.set_saved_query_sql_text("SELECT 43 AS [Answer];".into()));
    let mut preview = Preview::new(Scene::Datasheet);
    preview.state = state;
    preview
}

#[test]
fn synthetic_save_and_close_save_preserve_dirty_sql_and_report_unavailable() {
    let mut preview = dirty_preview();
    let tab = preview.state.active_object().unwrap();
    let request = preview.state.saved_query_sql_review_request().unwrap();
    preview.dispatch(Action::ReviewSavedQuerySql { request });
    assert!(preview.state.saved_query_sql_dirty());
    assert!(!preview.state.saved_query_sql_pending());
    assert!(matches!(
        preview.state.saved_query_sql_error(),
        Some(crate::SavedQuerySqlError::Host(_))
    ));
    assert!(preview.state.object_tabs().contains(&tab));
    assert!(preview.state.request_saved_query_sql_close(&tab));
    let result = preview
        .state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Save)
        .unwrap();
    preview.dispatch(Action::SavedQuerySqlClose { result });
    assert!(preview.state.saved_query_sql_dirty());
    assert_eq!(
        preview.state.saved_query_sql().unwrap().1,
        "SELECT 43 AS [Answer];"
    );
    assert!(preview.state.object_tabs().contains(&tab));
    assert!(!preview.state.saved_query_sql_pending());
}

#[test]
fn synthetic_close_cancel_keeps_the_object_and_discard_closes_only_its_exact_tab() {
    let mut preview = dirty_preview();
    let tab = preview.state.active_object().unwrap();
    assert!(preview.state.request_saved_query_sql_close(&tab));
    let result = preview
        .state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Cancel)
        .unwrap();
    preview.dispatch(Action::SavedQuerySqlClose { result });
    assert!(preview.state.saved_query_sql_dirty());
    assert!(preview.state.object_tabs().contains(&tab));
    assert!(preview.state.request_saved_query_sql_close(&tab));
    let result = preview
        .state
        .choose_saved_query_sql_close(SavedQuerySqlCloseDecision::Discard)
        .unwrap();
    assert!(!preview.state.saved_query_sql_dirty());
    preview.dispatch(Action::SavedQuerySqlClose { result });
    assert!(!preview.state.object_tabs().contains(&tab));
}

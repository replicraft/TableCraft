use super::super::Scene;
use super::*;
use crate::{Action, Data};

fn identity(data: &Data) -> (Option<u64>, String, u32, usize, usize) {
    (
        data.source_revision,
        data.table.clone(),
        data.offset,
        data.columns.len(),
        data.rows.len(),
    )
}

#[test]
fn query_pending_and_error_samples_preserve_source_and_read_only_result_boundaries() {
    for pending in [true, false] {
        let mut preview = Preview::new(Scene::Datasheet);
        preview.state.path = "/synthetic/never-opened/query.accdb".into();
        let original_table = identity(preview.state.data.as_ref().unwrap());
        let original_sql = preview.state.sql.clone();
        if pending {
            preview.prepare_query_running();
        } else {
            preview.prepare_query_error();
        }

        assert!(preview.state.view == DocumentView::Query);
        assert_eq!(preview.state.busy, pending);
        assert_eq!(
            identity(preview.state.data.as_ref().unwrap()),
            original_table
        );
        assert_eq!(preview.state.sql, original_sql);
        let result = preview.state.query_data.as_ref().unwrap();
        let before = identity(result);
        let rows = result.rows.as_ptr();
        assert_eq!(result.source_revision, None);
        assert!(!result.has_more);
        assert!(result.edit_target(0, 0).is_none());
        assert!(result.contents_target(0, 0).is_none());
        if pending {
            assert!(!preview.state.query_ready());
            preview.dispatch(Action::Cancel);
            assert!(!preview.state.busy);
            assert!(preview.state.query_ready());
        } else {
            assert!(preview.state.error.is_some());
        }
        let result = preview.state.query_data.as_ref().unwrap();
        assert_eq!(identity(result), before);
        assert_eq!(result.rows.as_ptr(), rows);
        assert_eq!(preview.state.localizer.diagnostics(), Default::default());
    }
}

#[test]
fn saved_rerun_and_failure_samples_retain_only_the_previous_complete_read_only_result() {
    for pending in [true, false] {
        let mut preview = Preview::new(Scene::Datasheet);
        let original_table = identity(preview.state.data.as_ref().unwrap());
        let original_sql = preview.state.sql.clone();
        if pending {
            preview.prepare_saved_running();
        } else {
            preview.prepare_saved_error();
        }

        let target = preview.state.selected_saved_target().unwrap().clone();
        assert_eq!(target, fixtures::saved_target());
        assert!(preview.state.saved_query_visible());
        assert_eq!(preview.state.busy, pending);
        assert_eq!(
            identity(preview.state.data.as_ref().unwrap()),
            original_table
        );
        assert_eq!(preview.state.sql, original_sql);
        assert!(preview.state.query_data.is_none());
        let result = preview.state.saved_query.data.as_ref().unwrap();
        let before = identity(result);
        let rows = result.rows.as_ptr();
        assert_eq!(result.source_revision, None);
        assert!(!result.has_more);
        assert!(result.edit_target(0, 0).is_none());
        assert!(result.contents_target(0, 0).is_none());
        if pending {
            assert!(!preview.state.saved_query_run_ready());
            preview.dispatch(Action::Cancel);
        }
        assert!(!preview.state.busy);
        assert!(preview.state.saved_query_run_ready());
        assert_eq!(preview.state.selected_saved_target(), Some(&target));
        let result = preview.state.saved_query.data.as_ref().unwrap();
        assert_eq!(identity(result), before);
        assert_eq!(result.rows.as_ptr(), rows);
        assert_eq!(preview.state.localizer.diagnostics(), Default::default());
    }
}

#[test]
fn saved_inspection_and_unselected_variant_prompts_never_admit_a_run_implicitly() {
    let mut preview = Preview::new(Scene::Datasheet);
    preview.prepare_saved_inspecting();
    assert!(preview.state.busy);
    assert!(!preview.state.saved_query_run_ready());
    assert!(preview.state.saved_query.data.is_none());
    preview.dispatch(Action::Cancel);
    assert!(!preview.state.busy);
    assert!(!preview.state.saved_query_run_ready());

    let mut preview = Preview::new(Scene::Datasheet);
    preview.prepare_saved_parameters();
    assert!(!preview.state.busy);
    assert!(preview.state.saved_query_visible());
    assert!(!preview.state.saved_query_run_ready());
    assert!(preview.state.saved_query_run_action().is_none());
    assert!(preview.state.saved_query.data.is_none());
    assert!(preview.state.query_data.is_none());
    assert_eq!(preview.state.sql, fixtures::SQL);
}

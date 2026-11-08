use super::super::Scene;
use super::*;
use crate::workspace::DocumentView;
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
        let table_tab = preview.state.active_object().unwrap();
        let original_table = identity(preview.state.data.as_ref().unwrap());
        let original_sql = preview.state.sql.clone();
        if pending {
            preview.prepare_query_running();
        } else {
            preview.prepare_query_error();
        }

        assert!(preview.state.view == DocumentView::Query);
        assert_eq!(preview.state.busy, pending);
        assert!(
            preview.state.data.is_none(),
            "native table is parked under its own tab"
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
        assert!(preview.state.activate_object(&table_tab));
        assert_eq!(
            identity(preview.state.data.as_ref().unwrap()),
            original_table
        );
    }
}

#[test]
fn saved_rerun_and_failure_samples_retain_only_the_previous_complete_read_only_result() {
    for pending in [true, false] {
        let mut preview = Preview::new(Scene::Datasheet);
        let table_tab = preview.state.active_object().unwrap();
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
        assert!(
            preview.state.data.is_none(),
            "native table is parked under its own tab"
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
        assert!(preview.state.activate_object(&table_tab));
        assert_eq!(
            identity(preview.state.data.as_ref().unwrap()),
            original_table
        );
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
    assert!(preview.state.saved_query_prompt_open());
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    for _ in 0..3 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 820.0),
                )),
                ..Default::default()
            },
            |ui| assert!(preview.state.show(ui).is_empty()),
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        let ok_label = preview.state.localizer.text("saved-query-prompt-ok");
        let ok = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.role() == egui::accesskit::Role::Button
                    && node.label() == Some(ok_label.as_str()))
                .then_some(node)
            })
            .expect("VALUE acquisition displays an explicit confirmation control");
        assert!(
            ok.is_disabled(),
            "VALUE requires a concrete kind before confirmation"
        );
        assert!(preview.state.saved_query_run_action().is_none());
        assert!(preview.state.saved_query.data.is_none());
        assert_eq!(preview.state.sql, fixtures::SQL);
    }
    assert_eq!(preview.state.localizer.diagnostics(), Default::default());
}

#[test]
fn scratch_preview_animation_and_cancel_preserve_previous_read_only_results() {
    for scene in [Scene::ScratchInspecting, Scene::ScratchParameters] {
        let mut preview = Preview::new(scene);
        let pending = scene == Scene::ScratchInspecting;
        assert_eq!(scene.is_animated(), pending);
        assert_eq!(preview.state.busy, pending);
        assert!(preview.state.query_results_previous());
        let data = preview.state.query_data.as_ref().unwrap();
        let before = identity(data);
        let rows = data.rows.as_ptr();
        assert!(data.source_revision.is_none());
        assert!(data.row_ordinals.is_none());
        assert!(data.edit_target(0, 0).is_none());

        preview.dispatch(Action::Cancel);
        assert!(!preview.state.busy);
        assert!(preview.state.query_parameters.is_none());
        let data = preview.state.query_data.as_ref().unwrap();
        assert_eq!(identity(data), before);
        assert_eq!(data.rows.as_ptr(), rows);
        assert_eq!(preview.state.localizer.diagnostics(), Default::default());
    }
}

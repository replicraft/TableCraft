use super::*;
use crate::preview::Scene;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn empty_design_scene_adds_only_explicit_synthetic_metadata_without_running_a_query() {
    let preview = Preview::new(Scene::QueryDesignEmpty);
    assert!(
        preview
            .state()
            .query_design_draft()
            .unwrap()
            .sources
            .is_empty()
    );
    assert!(
        preview
            .state()
            .query_design_draft()
            .unwrap()
            .columns
            .is_empty()
    );
    assert!(!preview.state().query_design_dialog_open());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 680.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
        .get_all_by_role_and_label(Role::Button, "Add Tables")
        .next()
        .expect("existing contextual Ribbon command")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().state().query_design_dialog_open());
    harness
        .get_by_role_and_label(Role::Button, "Add")
        .click_accesskit();
    harness.run_steps(3);
    let state = harness.state().state();
    let draft = state.query_design_draft().unwrap();
    assert_eq!(draft.sources.len(), 1);
    assert_eq!(draft.sources[0].table, "Customers");
    assert!(draft.joins.is_empty());
    assert!(draft.columns.is_empty());
    assert!(state.query_data.is_none());
    assert!(!state.busy);
}

#[test]
fn wide_design_scene_retains_hidden_criteria_and_explicit_join() {
    let preview = Preview::new(Scene::QueryDesignWide);
    let state = preview.state();
    let draft = state.query_design_draft().unwrap();
    assert_eq!(draft.columns.len(), 32);
    assert_eq!(draft.sources.len(), 2);
    assert_eq!(draft.joins.len(), 1);
    assert_eq!(draft.joins[0].kind, DesignJoinKind::Left);
    assert!(!draft.columns[1].show);
    assert_eq!(draft.columns[1].criteria.len(), 8);
    assert_eq!(
        draft.columns[31].alias.as_deref(),
        Some("Synthetic_projection_with_a_long_caption_31")
    );
    assert!(state.sql.contains("LEFT JOIN"));
    assert!(state.sql.contains("ORDER BY"));
    assert_eq!(draft.sources[0].table, "Customers");
    assert_eq!(draft.sources[1].table, "Orders");
    assert!(state.query_data.is_none());
    assert!(!state.busy);
}

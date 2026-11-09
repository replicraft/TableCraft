use crate::QueryDesignCompletion;
use crate::preview::{Preview, Scene};
use crate::workspace::{Action as WorkspaceAction, State};
use egui::{
    Event,
    accesskit::{Action, ActionRequest, Role, TreeId},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::sync::Arc;
use tablecraft_builders::query::build;

#[test]
fn ribbon_owns_query_views_and_requests_checked_sql_without_implicit_run() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<WorkspaceAction>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, "Run query")
            .count(),
        1
    );
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, "Add Tables")
            .count(),
        1
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Design View")
            .is_none()
    );

    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Design View")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Datasheet")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(3);
    let WorkspaceAction::BuildQueryDesign {
        request,
        draft,
        completion,
    } = harness.state_mut().1.remove(0)
    else {
        panic!("SQL View must request the existing checked builder");
    };
    assert_eq!(completion, QueryDesignCompletion::ShowSql);
    let metadata = harness
        .state()
        .0
        .query_design
        .session
        .as_ref()
        .unwrap()
        .metadata
        .clone();
    let built = build(&draft, &metadata).unwrap();
    let submitted = Arc::clone(&draft);
    assert!(
        harness
            .state_mut()
            .0
            .begin_query_design_build(request, draft, completion, 900)
    );
    let pending = &harness
        .state()
        .0
        .query_design
        .session
        .as_ref()
        .unwrap()
        .pending;
    let super::super::PendingKind::Build { draft, .. } = &pending.as_ref().unwrap().kind else {
        panic!("the normal Ribbon action must retain the submitted build");
    };
    assert!(Arc::ptr_eq(draft, &submitted));
    assert!(!std::ptr::eq(
        harness.state().0.query_design_draft().unwrap(),
        submitted.as_ref(),
    ));
    assert_eq!(
        harness
            .state_mut()
            .0
            .set_query_design_built(900, built, metadata),
        Some(QueryDesignCompletion::ShowSql)
    );
    harness.run_steps(3);
    assert!(!harness.state().0.query_design_visible());
    assert!(harness.query_by_role(Role::MultilineTextInput).is_some());
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, "Run query")
            .count(),
        1
    );
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(3);
    assert!(matches!(
        harness.state().1.as_slice(),
        [WorkspaceAction::Query]
    ));
    harness.state_mut().1.clear();

    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, "Run query")
            .count(),
        1
    );
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(3);
    assert!(matches!(
        harness.state().1.as_slice(),
        [WorkspaceAction::Query]
    ));
}

#[test]
fn bare_sql_view_menu_can_visit_cached_results_and_return_without_a_design_import() {
    let mut preview = Preview::new(Scene::Query);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<WorkspaceAction>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let sql = harness.state().0.sql.clone();
    let rows = harness.state().0.query_data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.query_design_results_visible());
    assert!(harness.query_by_role(Role::MultilineTextInput).is_none());
    harness
        .get_by_role_and_label(Role::Tab, "Query Design")
        .click_accesskit();
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Run query")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.query_by_role(Role::MultilineTextInput).is_some());
    assert_eq!(harness.state().0.sql, sql);
    assert_eq!(
        harness.state().0.query_data.as_ref().unwrap().rows.as_ptr(),
        rows
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn query_design_unavailable_commands_are_visible_in_both_hosts_and_cannot_dispatch() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    let original = state.query_design_draft().unwrap().clone();
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut render = |events| {
        let mut actions = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| actions.extend(state.show(ui)),
        );
        output.textures_delta.clear();
        (actions, output)
    };
    render(Vec::new());
    let (_, output) = render(Vec::new());
    let tree = output.platform_output.accesskit_update.unwrap();
    let mut forced = Vec::new();
    for label in ["Builder", "Totals", "Parameters", "Property Sheet"] {
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some(label))
            .expect("visible mock command");
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
        forced.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: *id,
            data: None,
        }));
    }
    let (actions, _) = render(forced);
    assert!(actions.is_empty());
    assert_eq!(state.query_design_draft(), Some(&original));

    state.mock_preview = false;
    let mut output = context.run_ui(egui::RawInput::default(), |ui| {
        assert!(state.show(ui).is_empty());
    });
    output.textures_delta.clear();
    let tree = output.platform_output.accesskit_update.unwrap();
    let mut forced = Vec::new();
    for label in [
        "Builder",
        "Totals",
        "Parameters",
        "Property Sheet",
        "Select",
        "Make Table",
        "Append",
        "Update",
        "Crosstab",
        "Delete",
        "Union",
        "Pass-Through",
        "Data Definition",
    ] {
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some(label))
            .expect("visible unavailable normal-host command");
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
        forced.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: *id,
            data: None,
        }));
    }
    let mut output = context.run_ui(
        egui::RawInput {
            events: forced,
            ..Default::default()
        },
        |ui| {
            assert!(state.show(ui).is_empty());
        },
    );
    output.textures_delta.clear();
    assert_eq!(state.query_design_draft(), Some(&original));
}

#[test]
fn normal_return_selector_exposes_localized_disabled_semantics_and_rejects_forced_activation() {
    let mut preview = Preview::new(Scene::QueryDesign);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = false;
    state.localizer = crate::Localizer::new(crate::Locale::De).unwrap();
    let label = state.localizer.text("ribbon-return");
    let value = state.localizer.text("ribbon-all");
    let description = state.localizer.text("ribbon-unavailable");
    assert_eq!(value, "Alle");
    let original_sql = state.sql.clone();
    let original_draft = state.query_design_draft().unwrap().clone();
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut render = |events| {
        let mut actions = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| actions.extend(state.show(ui)),
        );
        output.textures_delta.clear();
        (actions, output)
    };
    render(Vec::new());
    let (_, output) = render(Vec::new());
    let tree = output.platform_output.accesskit_update.unwrap();
    let (id, selector) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::ComboBox && node.label() == Some(label.as_str()))
        .expect("actual localized Return selector in the normal Ribbon");
    assert_eq!(selector.value(), Some(value.as_str()));
    assert_eq!(selector.description(), Some(description.as_str()));
    assert!(selector.is_disabled());
    assert!(!selector.supports_action(Action::Click));
    assert!(!selector.supports_action(Action::Focus));

    let events = [Action::Focus, Action::Click]
        .into_iter()
        .map(|action| {
            Event::AccessKitActionRequest(ActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: *id,
                data: None,
            })
        })
        .collect();
    let (actions, _) = render(events);
    assert!(actions.is_empty());
    assert!(!egui::Popup::is_any_open(&context));
    assert_ne!(
        context
            .memory(|memory| memory.focused())
            .map(|id| id.accesskit_id()),
        Some(*id)
    );
    assert_eq!(state.sql, original_sql);
    assert_eq!(state.query_design_draft(), Some(&original_draft));
    assert!(state.query_data.is_none());
}

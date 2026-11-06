use crate::preview::{Preview, Scene};
use egui::{
    Event,
    accesskit::{Action, ActionRequest, Role, TreeId},
};

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

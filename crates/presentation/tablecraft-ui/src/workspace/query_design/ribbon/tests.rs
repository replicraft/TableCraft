use crate::preview::{Preview, Scene};
use egui::{
    Event,
    accesskit::{Action, ActionRequest, Role, TreeId},
};

#[test]
fn query_design_preview_commands_are_disabled_and_cannot_dispatch() {
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
    assert!(!tree.nodes.iter().any(|(_, node)| {
        node.role() == Role::Button
            && matches!(
                node.label(),
                Some("Builder" | "Totals" | "Parameters" | "Property Sheet")
            )
    }));
}

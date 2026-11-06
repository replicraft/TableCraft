use super::*;
use crate::{Locale, Localizer};
use egui::{
    Event,
    accesskit::{Action, ActionRequest, Role, TreeId},
};

#[test]
fn unavailable_formatting_controls_have_meaningful_names_values_and_no_actions() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let localizer = Localizer::new(Locale::EnUs).unwrap();
    crate::localization::install(&context, &localizer);
    let render = |events| {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 220.0),
                )),
                events,
                ..Default::default()
            },
            |ui| home(ui, &localizer),
        );
        output.textures_delta.clear();
        output
    };
    render(Vec::new());
    let output = render(Vec::new());
    let tree = output.platform_output.accesskit_update.unwrap();
    let mut requests = Vec::new();
    for (label, value) in [("Font", "Selawik"), ("Font Size", "11")] {
        let (id, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::ComboBox && node.label() == Some(label))
            .expect("named font control");
        assert_eq!(node.value(), Some(value));
        assert_eq!(node.description(), Some("Not available yet"));
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
        requests.push(Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: *id,
            data: None,
        }));
    }
    for key in [
        "ribbon-bullets",
        "ribbon-numbering",
        "ribbon-decrease-indent",
        "ribbon-increase-indent",
        "ribbon-text-direction",
        "ribbon-bold",
        "ribbon-italic",
        "ribbon-underline",
        "ribbon-font-color",
        "ribbon-text-highlight",
        "ribbon-fill-color",
        "ribbon-align-left",
        "ribbon-align-center",
        "ribbon-align-right",
        "ribbon-borders",
        "ribbon-alternate-rows",
    ] {
        let label = localizer.text(key);
        let (_, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some(label.as_str()))
            .expect("screenshot formatting command");
        assert!(node.is_disabled());
        assert_eq!(node.description(), Some("Not available yet"));
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
    }
    let launcher = tree
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == Role::Button
                && node
                    .label()
                    .is_some_and(|label| label.starts_with("Text Formatting:"))
        })
        .expect("group launcher");
    assert_eq!(launcher.1.description(), Some("Not available yet"));
    assert!(launcher.1.is_disabled());

    let output = render(requests);
    assert!(!egui::Popup::is_any_open(&context));
    assert!(output.platform_output.commands.is_empty());
    assert!(context.memory(|memory| memory.focused()).is_none());
}

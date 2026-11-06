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
            |ui| home(ui, &localizer, crate::FontFamilies::default().families()),
        );
        output.textures_delta.clear();
        output
    };
    render(Vec::new());
    let output = render(Vec::new());
    let tree = output.platform_output.accesskit_update.unwrap();
    let mut requests = Vec::new();
    {
        let (label, value) = ("Font Size", "11");
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

#[test]
fn font_selectors_share_bounds_and_leave_space_before_the_formatting_row() {
    use egui_kittest::{Harness, kittest::Queryable};

    for scale in [1.0, 1.25, 1.5, 2.0] {
        let families = crate::FontFamilies::default();
        let localizer = Localizer::new(Locale::EnUs).unwrap();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 180.0))
            .with_pixels_per_point(scale)
            .build_ui(move |ui| {
                crate::localization::install(ui.ctx(), &localizer);
                home(ui, &localizer, families.families());
            });
        crate::configure(&harness.ctx);
        harness.run_steps(3);
        let family = harness.get_by_role_and_label(Role::ComboBox, "Font").rect();
        let size = harness
            .get_by_role_and_label(Role::ComboBox, "Font Size")
            .rect();
        let bold = harness.get_by_role_and_label(Role::Button, "Bold").rect();
        assert!((family.top() - size.top()).abs() < 0.1);
        assert!((family.bottom() - size.bottom()).abs() < 0.1);
        assert!(bold.top() - family.bottom() >= FONT_ROW_GAP - 0.1);
    }
}

#[test]
fn font_catalog_lists_host_families_without_applying_unavailable_formatting() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    let families =
        crate::FontFamilies::from_system(["Example Serif", "Literal Font Name"]).unwrap();
    let localizer = Localizer::new(Locale::EnUs).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(650.0, 400.0))
        .build_ui(move |ui| {
            crate::localization::install(ui.ctx(), &localizer);
            home(ui, &localizer, families.families());
        });
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let selector = harness.get_by_role_and_label(Role::ComboBox, "Font");
    assert!(!selector.accesskit_node().is_disabled());
    selector.click_accesskit();
    harness.run_steps(3);
    for family in ["Example Serif", "Literal Font Name", "Selawik"] {
        let entry = harness.get_by_role_and_label(Role::Button, family);
        assert!(entry.accesskit_node().is_disabled());
        entry.click_accesskit();
    }
    harness.run_steps(3);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Font")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Selawik")
    );
    assert!(harness.output().platform_output.commands.is_empty());
}

#[test]
fn large_font_catalog_has_one_scrollbar_and_renders_only_its_visible_rows() {
    use egui_kittest::{Harness, kittest::Queryable};

    let names = (0..500).map(|index| format!("Example family {index:04}"));
    let families = crate::FontFamilies::from_system(names).unwrap();
    let localizer = Localizer::new(Locale::EnUs).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(650.0, 420.0))
        .build_ui(move |ui| {
            crate::localization::install(ui.ctx(), &localizer);
            home(ui, &localizer, families.families());
        });
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::ComboBox, "Font")
        .click_accesskit();
    harness.run_steps(3);
    assert_eq!(harness.get_all_by_role(Role::ScrollBar).count(), 1);
    assert!(harness.get_all_by_role(Role::Button).count() < 100);
    harness.get_by_role_and_label(Role::Button, "Example family 0000");
}

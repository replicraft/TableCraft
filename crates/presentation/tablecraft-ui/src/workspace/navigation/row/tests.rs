use super::*;
use crate::Theme;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn focused_selected_system_badge_uses_readable_ink_without_losing_the_accent_ring() {
    for theme in [Theme::Light, Theme::Dark] {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui_state(
                |ui, selected: &mut bool| {
                    show(ui, Icon::Table, "MSysObjects", *selected, Some("System"));
                },
                true,
            );
        crate::configure(&harness.ctx);
        crate::theme::set_theme(&harness.ctx, theme);
        harness.run();
        let colors = palette(&harness.ctx);
        let badge_color = |output: &egui::FullOutput| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "System" => {
                        Some(text.fallback_color)
                    }
                    _ => None,
                })
                .expect("the actual system badge is painted")
        };
        assert_eq!(badge_color(harness.output()), colors.muted);
        harness
            .get_by_role_and_label(Role::Button, "MSysObjects")
            .focus();
        harness.run();
        assert_eq!(badge_color(harness.output()), colors.ink);
        let accent_focus_ring = harness
            .output()
            .shapes
            .iter()
            .any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => rect.stroke.color == colors.accent,
                _ => false,
            });
        let selected_background = harness
            .output()
            .shapes
            .iter()
            .any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => rect.fill == colors.active_row,
                _ => false,
            });
        assert!(accent_focus_ring);
        assert!(selected_background);

        *harness.state_mut() = false;
        harness.run();
        assert_eq!(badge_color(harness.output()), colors.muted);
    }
}

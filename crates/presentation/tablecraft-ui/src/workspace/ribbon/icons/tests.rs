use super::*;

#[test]
fn every_original_icon_decodes_at_command_sizes_in_both_themes_and_enabled_states() {
    let context = egui::Context::default();
    crate::configure(&context);
    for theme in [crate::Theme::Light, crate::Theme::Dark] {
        crate::theme::set_theme(&context, theme);
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            for enabled in [true, false] {
                ui.add_enabled_ui(enabled, |ui| {
                    for icon in Icon::ALL {
                        for size in [16.0, 28.0] {
                            let loaded = icon
                                .image(ui, size)
                                .load_for_size(ui.ctx(), egui::vec2(size, size));
                            assert!(
                                matches!(loaded, Ok(egui::load::TexturePoll::Ready { .. })),
                                "embedded original SVG must decode synchronously"
                            );
                        }
                    }
                });
            }
        });
        output.textures_delta.clear();
    }
}

//! Presentation-only preview host. It owns no database worker or file resources.
use tablecraft_ui::{
    Localizer, Theme,
    preview::{Preview, Scene},
};

pub(super) struct MockApp {
    preview: Preview,
    startup_visibility_pending: bool,
}

impl MockApp {
    pub(super) fn new(scene: Scene, localizer: Localizer, theme: Theme) -> Self {
        let mut preview = Preview::new(Scene::Datasheet);
        preview.state_mut().localizer = localizer;
        preview.state_mut().theme = theme;
        preview.set_scene(scene);
        preview.state_mut().formatting.system_locale = sys_locale::get_locale();

        Self {
            preview,
            startup_visibility_pending: true,
        }
    }
}

impl eframe::App for MockApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.startup_visibility_pending) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.preview.show(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_ui::Locale;

    #[test]
    fn initial_scene_messages_use_the_requested_language() {
        let localizer = Localizer::new(Locale::Ja).unwrap();
        let expected = localizer.text("settings-mock-retained");
        let english = Localizer::default().text("settings-mock-retained");
        let mut app = MockApp::new(Scene::Recovery, localizer, Theme::Dark);
        let context = egui::Context::default();
        tablecraft_ui::configure(&context);

        let mut painted = Vec::new();
        for _ in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| app.preview.show(ui),
            );
            painted = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            output.textures_delta.clear();
        }
        assert!(painted.contains(&expected));
        assert!(!painted.contains(&english));
    }
}

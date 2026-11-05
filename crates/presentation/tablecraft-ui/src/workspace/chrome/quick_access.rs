//! Document commands remain unavailable until their owning service can admit them.
use crate::Localizer;
use crate::workspace::ribbon::Icon;
use egui::{Ui, WidgetInfo, WidgetType};

const BUTTON_SIZE: f32 = 20.0;
const ICON_SIZE: f32 = 16.0;

pub(super) fn show(ui: &mut Ui, localizer: &Localizer) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
        ui.spacing_mut().interact_size = egui::Vec2::splat(BUTTON_SIZE);
        // Native saves are consuming operations in the reviewed edit dialog.
        // There is no inline dirty record or document undo/redo service to invoke.
        // TextEdit owns its independent local history and keyboard commands.
        for (key, help, icon) in [
            ("ribbon-save", "shell-save-unavailable", Icon::Save),
            ("ribbon-undo", "shell-undo-unavailable", Icon::Undo),
            ("ribbon-redo", "shell-redo-unavailable", Icon::Redo),
        ] {
            let label = localizer.text(key);
            let response = ui
                .push_id(("quick_access", key), |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        ui.add(egui::Button::image(icon.image(ui, ICON_SIZE)).frame(false))
                    })
                    .inner
                })
                .inner;
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, false, &label));
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.remove_action(egui::accesskit::Action::Click);
                node.remove_action(egui::accesskit::Action::Focus);
            });
            response.on_disabled_hover_text(localizer.text(help));
        }
    });
}

//! Quick Access visibility is presentation state; unsupported document commands stay inert.
use crate::Localizer;
use crate::workspace::Action;
use crate::workspace::ribbon::Icon;
use egui::{Id, Ui, WidgetInfo, WidgetType};

const BUTTON_SIZE: f32 = 24.0;
const ICON_SIZE: f32 = 16.0;
const BUTTON_GAP: f32 = 7.0;

#[derive(Clone, Copy)]
struct Visibility {
    open: bool,
    save: bool,
    undo: bool,
    redo: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Self {
            open: true,
            save: true,
            undo: true,
            redo: true,
        }
    }
}

pub(super) fn show(
    ui: &mut Ui,
    localizer: &Localizer,
    open_available: bool,
    actions: &mut Vec<Action>,
) {
    let id = Id::new("quick_access_visibility");
    let mut visibility = ui
        .data(|data| data.get_temp::<Visibility>(id))
        .unwrap_or_default();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP;
        ui.spacing_mut().button_padding = egui::vec2(3.0, 2.0);
        ui.spacing_mut().interact_size = egui::Vec2::splat(BUTTON_SIZE);
        if visibility.open {
            let label = localizer.text("shell-quick-open");
            let response = ui.add_enabled(
                open_available,
                egui::Button::image(Icon::FolderOpen.image(ui, ICON_SIZE)).frame(false),
            );
            response
                .widget_info(|| WidgetInfo::labeled(WidgetType::Button, open_available, &label));
            if response.on_hover_text(label).clicked() {
                actions.push(Action::ChooseFile);
            }
        }
        // Native persistence is owned by the consuming Review/Apply workflow.
        // TextEdit retains its own local undo/redo; this toolbar has no document history.
        for (visible, key, help, icon) in [
            (
                visibility.save,
                "ribbon-save",
                "shell-save-unavailable",
                Icon::Save,
            ),
            (
                visibility.undo,
                "ribbon-undo",
                "shell-undo-unavailable",
                Icon::Undo,
            ),
            (
                visibility.redo,
                "ribbon-redo",
                "shell-redo-unavailable",
                Icon::Redo,
            ),
        ] {
            if !visible {
                continue;
            }
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
        let menu = ui.menu_button(Icon::QuickAccess.image(ui, 12.0), |ui| {
            ui.label(localizer.text("shell-quick-access"));
            ui.separator();
            for (visible, key) in [
                (&mut visibility.open, "backstage-open"),
                (&mut visibility.save, "ribbon-save"),
                (&mut visibility.undo, "ribbon-undo"),
                (&mut visibility.redo, "ribbon-redo"),
            ] {
                ui.checkbox(visible, localizer.text(key));
            }
        });
        menu.response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                true,
                localizer.text("shell-quick-access"),
            )
        });
        menu.response
            .on_hover_text(localizer.text("shell-quick-access"));
    });
    ui.data_mut(|data| data.insert_temp(id, visibility));
}

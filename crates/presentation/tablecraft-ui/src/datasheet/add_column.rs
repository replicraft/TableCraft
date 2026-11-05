//! Presentation-only native datasheet header, never a stored field or cell.
use crate::localization::tr;
use egui::{Button, RichText, Ui};

pub(super) const WIDTH: f32 = 128.0;

pub(super) fn header(ui: &mut Ui) {
    super::grid_cell(ui, true);
    let label = tr(ui, "datasheet-click-to-add");
    let unavailable = tr(ui, "datasheet-add-field-unavailable");
    let button = Button::new(RichText::new(label).italics())
        .frame(false)
        .truncate();
    ui.add_enabled_ui(false, |ui| {
        ui.add_sized(ui.available_size(), button)
            .on_disabled_hover_text(unavailable);
    });
}

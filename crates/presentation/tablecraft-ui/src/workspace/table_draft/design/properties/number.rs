//! Number-size selection edits the same unsaved declaration as the upper type grid.
use super::Editor;
use crate::localization::tr;
use crate::workspace::field_properties::sheet::Sheet;
use crate::workspace::field_properties::{number_size, unsigned_input};
use egui::Ui;
use tablecraft_builders::table::DraftNumberSize;

pub(super) fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    position: usize,
    size: DraftNumberSize,
    primary: bool,
    sheet: &mut Sheet,
) {
    let value_width = sheet.value_width();
    sheet.label(ui, "design-field-size", true);
    let next = number_size(
        ui,
        size,
        |candidate| !primary || candidate == DraftNumberSize::LongInteger,
        value_width,
    );
    sheet.end_row(ui);

    // Apply after painting the current pair so a same-frame raw edit cannot be
    // hidden by a size selection. Rejected pairs keep the existing size selected.
    if size == DraftNumberSize::Decimal {
        for (precision, key) in [
            (true, "table-draft-number-precision"),
            (false, "table-draft-number-scale"),
        ] {
            sheet.label(ui, key, true);
            let mut raw = editor
                .properties
                .decimal_text(position, precision)
                .to_owned();
            let (input, rejected) = unsigned_input(ui, key, key, &mut raw, value_width);
            if input.changed() || rejected {
                editor.properties.set_decimal_text(
                    &mut editor.draft,
                    position,
                    precision,
                    raw,
                    rejected,
                );
            }
            if editor.properties.decimal_rejected(position) {
                ui.ctx().accesskit_node_builder(input.id, |node| {
                    node.set_invalid(egui::accesskit::Invalid::True)
                });
                input.on_hover_text(tr(ui, "table-draft-number-invalid-decimal"));
            }
            sheet.end_row(ui);
        }
    }
    if let Some(size) = next {
        editor
            .properties
            .set_number_size(&mut editor.draft, position, size);
    }
}

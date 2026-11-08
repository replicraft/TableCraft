//! Number-size selection edits the same unsaved declaration as the upper type grid.
use super::{Editor, PROPERTY_INPUT_MARGIN, editor::MAX_SIZE_INPUT_BYTES, sheet::Sheet};
use crate::localization::tr;
use egui::{Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::DraftNumberSize;

const SIZES: [(DraftNumberSize, &str); 7] = [
    (DraftNumberSize::Byte, "table-draft-number-byte"),
    (DraftNumberSize::Integer, "table-draft-number-integer"),
    (DraftNumberSize::LongInteger, "design-size-long-integer"),
    (DraftNumberSize::Single, "table-draft-number-single"),
    (DraftNumberSize::Double, "table-draft-number-double"),
    (DraftNumberSize::Decimal, "table-draft-number-decimal"),
    (
        DraftNumberSize::ReplicationId,
        "table-draft-number-replication-id",
    ),
];

pub(super) fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    position: usize,
    size: DraftNumberSize,
    sheet: &mut Sheet,
) {
    let value_width = sheet.value_width();
    sheet.label(ui, "design-field-size", true);
    let selected_key = SIZES
        .iter()
        .find(|(candidate, _)| *candidate == size)
        .unwrap()
        .1;
    let mut next = None;
    let response = egui::ComboBox::from_id_salt("number_size")
        .width(value_width)
        .selected_text(tr(ui, selected_key))
        .show_ui(ui, |ui| {
            for (candidate, key) in SIZES {
                if ui
                    .selectable_label(candidate == size, tr(ui, key))
                    .clicked()
                {
                    next = Some(candidate);
                    ui.close();
                }
            }
        })
        .response;
    let label = tr(ui, "design-field-size");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), &label));
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
            let id = ui.make_persistent_id(key);
            let rejected = crate::text_input::reject_events_with_limits(
                ui,
                id,
                &raw,
                MAX_SIZE_INPUT_BYTES,
                MAX_SIZE_INPUT_BYTES,
            );
            let input = ui.add(
                egui::TextEdit::singleline(&mut raw)
                    .id(id)
                    .margin(PROPERTY_INPUT_MARGIN)
                    .desired_width(value_width),
            );
            let label = tr(ui, key);
            input
                .widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, input.enabled(), &label));
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

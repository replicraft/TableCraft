//! Shared declaration controls for initial and saved-field drafts.
pub(in crate::workspace) mod sheet;

use crate::localization::tr;
use egui::{Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::DraftNumberSize;
use tablecraft_schema::ScalarType;

pub(in crate::workspace) const PROPERTY_INPUT_MARGIN: egui::Margin = egui::Margin::symmetric(4, 2);
pub(in crate::workspace) const MAX_SIZE_INPUT_BYTES: usize = 64;

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

pub(in crate::workspace) fn number_size_for_type(kind: ScalarType) -> Option<DraftNumberSize> {
    SIZES
        .iter()
        .map(|(size, _)| *size)
        .find(|size| size.scalar_type() == kind)
}

pub(in crate::workspace) fn number_size(
    ui: &mut Ui,
    size: DraftNumberSize,
    permitted: impl Fn(DraftNumberSize) -> bool,
    value_width: f32,
) -> Option<DraftNumberSize> {
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
                    .add_enabled(
                        permitted(candidate),
                        egui::Button::new(tr(ui, key)).selected(candidate == size),
                    )
                    .on_disabled_hover_text(tr(ui, "ribbon-unavailable"))
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
    next
}

pub(in crate::workspace) fn boolean_property(
    ui: &mut Ui,
    id: &str,
    label: &str,
    current: Option<bool>,
    enabled: bool,
    value_width: f32,
) -> Option<bool> {
    let mut selected = None;
    ui.add_enabled_ui(enabled, |ui| {
        let response = egui::ComboBox::from_id_salt(id)
            .width(value_width)
            .selected_text(tr(ui, flag_key(current)))
            .show_ui(ui, |ui| {
                for value in [false, true] {
                    if ui
                        .selectable_label(current == Some(value), tr(ui, flag_key(Some(value))))
                        .clicked()
                    {
                        selected = Some(value);
                        ui.close();
                    }
                }
            })
            .response;
        let accessible_label = tr(ui, label);
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), &accessible_label)
        });
        if !enabled {
            response.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
        }
    });
    selected
}

fn flag_key(value: Option<bool>) -> &'static str {
    match value {
        None => "table-draft-property-unset",
        Some(false) => "table-draft-property-no",
        Some(true) => "table-draft-property-yes",
    }
}

/// Paint the same bounded raw scalar editor; declaration validation stays with
/// the caller so incomplete text never becomes a valid field request.
pub(in crate::workspace) fn unsigned_input(
    ui: &mut Ui,
    id: &str,
    label: &str,
    raw: &mut String,
    width: f32,
) -> (egui::Response, bool) {
    let id = ui.make_persistent_id(id);
    let rejected = crate::text_input::reject_events_with_limits(
        ui,
        id,
        raw,
        MAX_SIZE_INPUT_BYTES,
        MAX_SIZE_INPUT_BYTES,
    );
    let response = ui.add(
        egui::TextEdit::singleline(raw)
            .id(id)
            .margin(PROPERTY_INPUT_MARGIN)
            .desired_width(width),
    );
    let label = tr(ui, label);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, response.enabled(), &label));
    (response, rejected)
}

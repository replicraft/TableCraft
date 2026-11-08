//! One Design type list; unavailable declarations remain visible and inert.
use crate::localization::tr;
use egui::Ui;
use tablecraft_builders::table::DraftFieldType;

const MENU_HEIGHT: f32 = 320.0;
const VIEWPORT_MARGIN: f32 = 24.0;
const MIN_MENU_HEIGHT: f32 = 80.0;
const MENU_WIDTH: f32 = 220.0;

const TYPES: [(Option<DraftFieldType>, &str); 14] = [
    (Some(DraftFieldType::ShortText), "design-type-short-text"),
    (None, "design-type-long-text"),
    (Some(DraftFieldType::Number), "design-type-number"),
    (
        Some(DraftFieldType::LargeInteger),
        "design-type-large-number",
    ),
    (Some(DraftFieldType::DateTime), "design-type-date-time"),
    (None, "design-type-date-time-extended"),
    (Some(DraftFieldType::Currency), "design-type-currency"),
    (None, "design-type-autonumber"),
    (Some(DraftFieldType::YesNo), "design-type-yes-no"),
    (None, "design-type-ole-object"),
    (None, "design-type-hyperlink"),
    (None, "design-type-attachment"),
    (None, "design-type-calculated"),
    (None, "design-type-lookup-wizard"),
];

pub(super) fn menu_height(ui: &Ui) -> f32 {
    (ui.ctx().content_rect().height() - VIEWPORT_MARGIN * 2.0).clamp(MIN_MENU_HEIGHT, MENU_HEIGHT)
}

// ComboBox owns the single bounded scroll area. Nested scrolling would clip
// later choices against an unrelated outer default height.
pub(super) fn show(
    ui: &mut Ui,
    current: DraftFieldType,
    allowed: impl Fn(DraftFieldType) -> bool,
) -> Option<DraftFieldType> {
    ui.set_min_width(MENU_WIDTH);
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut selected = None;
    for (kind, key) in TYPES {
        let available = kind.is_some_and(&allowed);
        let response = ui.add_enabled(
            available,
            egui::Button::new(tr(ui, key))
                .selected(kind == Some(current))
                .frame_when_inactive(kind == Some(current))
                .min_size(egui::vec2(
                    ui.available_width(),
                    ui.spacing().interact_size.y,
                ))
                .wrap(),
        );
        if response.clicked() {
            selected = kind;
            ui.close();
        }
        response.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
    }
    selected
}

//! Bounded Click-to-Add choices for the qualified initial-table authoring profile.
use crate::{localization::tr, workspace::ribbon::Icon};
use egui::{Button, Response, Ui};
use tablecraft_builders::table::DraftFieldType;

const MENU_WIDTH: f32 = 228.0;
const MIN_MENU_WIDTH: f32 = 160.0;
const MAX_MENU_HEIGHT: f32 = 360.0;
const VIEWPORT_MARGIN: f32 = 24.0;
const ROW_HEIGHT: f32 = 22.0;
const ROW_GAP: f32 = 2.0;
const ICON_SIZE: f32 = 16.0;
const CONTENT_PADDING: f32 = 6.0;
const ICON_GAP: f32 = 6.0;

const AVAILABLE: [(DraftFieldType, &str); 6] = [
    (DraftFieldType::ShortText, "table-draft-short-text"),
    (DraftFieldType::Number, "table-draft-number"),
    (DraftFieldType::LargeInteger, "table-draft-large-integer"),
    (DraftFieldType::Currency, "table-draft-currency"),
    (DraftFieldType::DateTime, "table-draft-date-time"),
    (DraftFieldType::YesNo, "table-draft-yes-no"),
];

const UNAVAILABLE: [&str; 7] = [
    "table-draft-long-text",
    "table-draft-lookup-relationship",
    "table-draft-rich-text",
    "table-draft-attachment",
    "table-draft-hyperlink",
    "table-draft-calculated-field",
    "table-draft-more-fields",
];

pub(super) fn show(ui: &mut Ui) -> Option<DraftFieldType> {
    let viewport = ui.ctx().content_rect();
    let width = (viewport.width() - VIEWPORT_MARGIN * 2.0).clamp(MIN_MENU_WIDTH, MENU_WIDTH);
    let height = (viewport.height() - VIEWPORT_MARGIN * 2.0).min(MAX_MENU_HEIGHT);
    ui.set_width(width);
    ui.spacing_mut().item_spacing.y = ROW_GAP;
    let mut selected = None;
    egui::ScrollArea::vertical()
        .id_salt("draft_field_gallery")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (kind, key) in AVAILABLE {
                if entry(ui, key, None).clicked() {
                    selected = Some(kind);
                    ui.close();
                }
            }
            ui.separator();
            for key in UNAVAILABLE {
                let response = ui
                    .add_enabled_ui(false, |ui| {
                        let icon = match key {
                            "table-draft-attachment" => Some(Icon::Attachment),
                            "table-draft-lookup-relationship" => Some(Icon::Lookups),
                            "table-draft-hyperlink" => Some(Icon::LinkedTables),
                            _ => None,
                        };
                        entry(ui, key, icon)
                    })
                    .inner;
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.remove_action(egui::accesskit::Action::Click);
                    node.remove_action(egui::accesskit::Action::Focus);
                });
                response.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
            }
        });
    selected
}

// Keep one full-width accessible button and a stable icon gutter. Painting its
// wrapped label explicitly avoids centering scalar labels or shifting them when
// another entry has an icon.
fn entry(ui: &mut Ui, key: &str, icon: Option<Icon>) -> Response {
    let label = tr(ui, key);
    let text_width = (ui.available_width() - CONTENT_PADDING * 2.0 - ICON_SIZE - ICON_GAP).max(1.0);
    let text_color = if ui.is_enabled() {
        ui.visuals().widgets.inactive.fg_stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let galley = ui.painter().layout(
        // Two real toolkit outputs retain the text independently (paint/layout, accessibility
        // or hover/clipboard); the owning API consumes the String while the other output
        // remains needed.
        label.clone(),
        egui::TextStyle::Button.resolve(ui.style()),
        text_color,
        text_width,
    );
    let height = ROW_HEIGHT.max(galley.size().y + CONTENT_PADDING);
    let response = ui.add_sized([ui.available_width(), height], Button::new(""));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
    });
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            response.rect.left() + CONTENT_PADDING + ICON_SIZE / 2.0,
            response.rect.center().y,
        ),
        egui::Vec2::splat(ICON_SIZE),
    );
    if let Some(icon) = icon {
        icon.image(ui, ICON_SIZE).paint_at(ui, icon_rect);
    }
    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + ICON_GAP,
            response.rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text_color,
    );
    response
}

#[cfg(test)]
mod tests;

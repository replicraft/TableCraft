//! Searchable family-name catalog; browsing never applies document formatting.
use crate::Localizer;
use egui::{Response, Ui};

const MENU_HEIGHT: f32 = 240.0;
const MIN_MENU_WIDTH: f32 = 200.0;
const MAX_SEARCH_CHARACTERS: usize = 128;

#[derive(Clone, Default)]
struct Search {
    query: String,
    open: bool,
}

pub(super) fn show(ui: &mut Ui, t: &Localizer, families: &[String], label_size: f32) -> Response {
    let search_id = ui.make_persistent_id("font_family_search");
    let mut search = ui
        .data(|data| data.get_temp::<Search>(search_id))
        .unwrap_or_default();
    let result = egui::ComboBox::from_id_salt("font_family_catalog")
        .width(120.0)
        .height(MENU_HEIGHT)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(egui::RichText::new("Selawik").size(label_size))
        .show_ui(ui, |ui| {
            ui.set_min_width(MIN_MENU_WIDTH);
            let search_label = t.text("ribbon-font-search");
            let response = ui.add(
                egui::TextEdit::singleline(&mut search.query)
                    .id(search_id.with("input"))
                    .hint_text(&search_label)
                    .desired_width(ui.available_width())
                    .char_limit(MAX_SEARCH_CHARACTERS),
            );
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label(search_label.as_str());
            });
            let reset_scroll = !search.open || response.changed();
            if !search.open {
                response.request_focus();
                search.open = true;
            }

            let query = search.query.trim().to_lowercase();
            let matches: Vec<_> = families
                .iter()
                .filter(|family| query.is_empty() || family.to_lowercase().contains(&query))
                .collect();
            if matches.is_empty() {
                ui.weak(t.text("ribbon-font-no-results"));
                return;
            }

            let row_height = ui.spacing().interact_size.y;
            let list_height = (MENU_HEIGHT - response.rect.height() - ui.spacing().item_spacing.y)
                .max(row_height);
            let mut list = egui::ScrollArea::vertical()
                .id_salt("font_family_catalog_rows")
                .max_height(list_height);
            if reset_scroll {
                list = list.vertical_scroll_offset(0.0);
            }
            list.show_rows(ui, row_height, matches.len(), |ui, range| {
                for family in &matches[range] {
                    ui.add_enabled(
                        false,
                        egui::Button::selectable(family.as_str() == "Selawik", family.as_str())
                            .truncate(),
                    )
                    .on_disabled_hover_text(t.text("ribbon-unavailable"));
                }
            });
        });
    if result.inner.is_none() {
        search = Search::default();
    }
    ui.data_mut(|data| data.insert_temp(search_id, search));
    result.response
}

#[cfg(test)]
mod tests;

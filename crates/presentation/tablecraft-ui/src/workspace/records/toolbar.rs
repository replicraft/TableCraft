//! Compact native-style record controls; every action retains its existing owner.
use super::{Action, RecordPositions, State, parse_record};
use crate::localization::{message, tr};
use crate::workspace::ribbon::Icon;
use egui::{Response, Ui, WidgetInfo, WidgetType};

const BUTTON_SIZE: f32 = 20.0;
const ICON_SIZE: f32 = 12.0;
const POSITION_WIDTH: f32 = 44.0;
const POSITION_TEXT_PADDING: f32 = 8.0;
const SEARCH_WIDTH: f32 = 120.0;
const MAX_RECORD_DIGITS: usize = 10;
const MAX_SEARCH_BYTES: usize = 64 * 1024;

impl State {
    pub(in crate::workspace) fn record_navigator(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        ui.horizontal(|ui| self.record_toolbar(ui, actions));
    }

    fn record_toolbar(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let current = self.selected_record_target();
        let positions = self.record_positions();
        let idle = !self.busy && self.records.pending.is_none() && !self.table_filter_pending();
        let enabled = idle && current.is_some();
        let mut destination = None;
        let mut search = false;
        let count = self
            .filtered_record_count()
            .map(u64::from)
            .or_else(|| self.checked_record_count())
            .or_else(|| {
                let data = self.data.as_ref()?;
                (!data.has_more).then_some(u64::from(data.offset) + data.rows.len() as u64)
            });

        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(2.0, 1.0);
        ui.label(tr(ui, "datasheet-record"));
        let (position, first, previous, next, last) = match positions {
            Some(RecordPositions {
                current,
                first,
                previous,
                next,
                last,
            }) => (u64::from(current) + 1, first, previous, next, last),
            None => (0, None, None, None, None),
        };
        for (icon, key, target) in [
            (Icon::FirstRecord, "datasheet-first", first),
            (Icon::PreviousRecord, "datasheet-previous", previous),
        ] {
            if record_button(ui, icon, key, enabled && target.is_some()).clicked() {
                destination = target;
            }
        }
        let input_id = ui.make_persistent_id("record_position");
        if !ui.memory(|memory| memory.has_focus(input_id)) {
            self.records.position = position.to_string();
        }
        if crate::text_input::reject_oversized_events(
            ui,
            input_id,
            &self.records.position,
            MAX_RECORD_DIGITS,
        ) {
            self.records.error = Some(tr(ui, "datasheet-record-input-help"));
        }
        let position_width = record_position_width(ui, &self.records.position, count);
        let input = ui.add_enabled(
            enabled,
            egui::TextEdit::singleline(&mut self.records.position)
                .id(input_id)
                .char_limit(MAX_RECORD_DIGITS)
                .desired_width(position_width),
        );
        let position_label = tr(ui, "datasheet-record-position");
        input.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, enabled, &position_label));
        if enabled
            && (input.has_focus() || input.lost_focus())
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            if let Some(ordinal) = parse_record(&self.records.position) {
                destination = Some(ordinal);
                input.surrender_focus();
            } else {
                self.records.error = Some(tr(ui, "datasheet-record-input-help"));
            }
        }
        let count = count.map_or_else(
            || tr(ui, "datasheet-count-unknown"),
            |count| count.to_string(),
        );
        ui.label(message(ui, "datasheet-record-of", &[("count", count)]));
        for (icon, key, target) in [
            (Icon::NextRecord, "datasheet-next", next),
            (Icon::LastRecord, "datasheet-last", last),
        ] {
            if record_button(ui, icon, key, enabled && target.is_some()).clicked() {
                destination = target;
            }
        }
        let insert = self.current_insert_target();
        if record_button(ui, Icon::NewRecord, "insertion-title", insert.is_some())
            .on_hover_text(message(
                ui,
                "insertion-shortcut-help",
                &[(
                    "shortcut",
                    crate::keyboard::new_record_shortcut_text(ui.ctx()),
                )],
            ))
            .clicked()
            && let Some(target) = insert
        {
            actions.push(Action::BeginInsert { target });
        }
        ui.separator();
        let filtered = self.table_filter_enabled();
        let filter_target = self.active_filter_target();
        let toggle = filter_target
            .as_ref()
            .is_some_and(|target| self.can_filter_page(target, 0, !filtered));
        let filter = ui
            .add_enabled(
                toggle,
                egui::Button::image_and_text(
                    if filtered {
                        Icon::Filter
                    } else {
                        Icon::NoFilter
                    }
                    .image(ui, ICON_SIZE),
                    tr(
                        ui,
                        if filtered {
                            "datasheet-filtered"
                        } else {
                            "datasheet-no-filter"
                        },
                    ),
                )
                .frame(false),
            )
            .on_hover_text(tr(ui, "filtering-toggle-help"));
        if filter.clicked()
            && let Some(target) = filter_target
        {
            actions.push(Action::FilterPage {
                target,
                offset: 0,
                enabled: !filtered,
            });
        }
        ui.separator();
        let search_id = ui.make_persistent_id("record_search");
        if crate::text_input::reject_oversized_events(
            ui,
            search_id,
            &self.records.search,
            MAX_SEARCH_BYTES,
        ) {
            self.records.error = Some(tr(ui, "find-pattern-limit"));
        }
        let search_input = ui.add_enabled(
            self.can_open_find(),
            egui::TextEdit::singleline(&mut self.records.search)
                .id(search_id)
                .char_limit(MAX_SEARCH_BYTES)
                .margin(egui::Margin {
                    left: 19,
                    right: 4,
                    top: 2,
                    bottom: 2,
                })
                .hint_text(tr(ui, "datasheet-search-records"))
                .desired_width(SEARCH_WIDTH),
        );
        let search_icon = egui::Rect::from_center_size(
            egui::pos2(search_input.rect.left() + 9.0, search_input.rect.center().y),
            egui::Vec2::splat(12.0),
        );
        Icon::Search.image(ui, 12.0).paint_at(ui, search_icon);
        let search_label = tr(ui, "datasheet-search-records");
        search_input.widget_info(|| {
            WidgetInfo::labeled(WidgetType::TextEdit, search_input.enabled(), &search_label)
        });
        search |= search_input.enabled()
            && (search_input.has_focus() || search_input.lost_focus())
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        // NO_CLONE: Search submission owns the pattern stored by the Find dialog; the toolbar
        // search field stays populated for subsequent submissions.
        if search && let Some(action) = self.find_from_record_search(self.records.search.clone()) {
            actions.push(action);
        }
        if self.records.pending.is_some() {
            ui.spinner();
        }
        if let Some(error) = &self.records.error {
            ui.label(error);
        }
        if let (Some(current), Some(destination)) = (current, destination) {
            self.request_display_record(current, destination, actions);
        }
    }
}

fn record_button(ui: &mut Ui, icon: Icon, key: &str, enabled: bool) -> Response {
    let label = tr(ui, key);
    let response = ui.add_enabled(
        enabled,
        egui::Button::image(icon.image(ui, ICON_SIZE))
            .frame(false)
            .min_size(egui::Vec2::splat(BUTTON_SIZE)),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
    response.on_hover_text(label)
}

fn record_position_width(ui: &Ui, position: &str, count: Option<u64>) -> f32 {
    let digits = position
        .chars()
        .count()
        .max(count.map_or(1, |count| count.checked_ilog10().unwrap_or(0) as usize + 1))
        .min(MAX_RECORD_DIGITS);
    let font = egui::FontSelection::Default.resolve(ui.style());
    let digit_width = ui.fonts_mut(|fonts| {
        ('0'..='9')
            .map(|digit| fonts.glyph_width(&font, digit))
            .fold(0.0_f32, f32::max)
    });
    POSITION_WIDTH.max(digit_width * digits as f32 + POSITION_TEXT_PADDING)
}

#[cfg(test)]
mod tests;

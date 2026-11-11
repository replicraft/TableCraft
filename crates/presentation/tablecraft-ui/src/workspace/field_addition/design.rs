//! The first blank Design row is one staged proposal, never a native field.
use super::header::{Event, Presentation};
use crate::localization::tr;
use crate::workspace::field_name::{NameBuffer, bounded_name};
use crate::workspace::{ObjectTab, State};
use egui::{TextBuffer, TextEdit, Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::{FieldAdditionKind, MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

const NAME_PADDING: f32 = 2.0;

pub(crate) fn name(ui: &mut Ui, presentation: &Presentation) -> Option<Event> {
    let original = presentation
        .name
        .as_ref()
        .map_or("", |(name, _)| name.as_str());
    let revision = presentation
        .name
        .as_ref()
        .map_or(0, |(_, revision)| *revision);
    let enabled = presentation.enabled
        && (presentation.name.is_some()
            || presentation.choices.supports(FieldAdditionKind::ShortText));
    let Some(mut text) = NameBuffer::new(original) else {
        return Some(Event::MemoryLimit);
    };
    let id = ui.make_persistent_id("saved_design_new_name");
    let rejected = crate::text_input::reject_events_with_limits(
        ui,
        id,
        original,
        MAX_DRAFT_NAME_BYTES,
        MAX_DRAFT_NAME_UNITS,
    );
    let response = ui.add_enabled(
        enabled,
        TextEdit::singleline(&mut text)
            .id(id)
            .desired_width(ui.available_width())
            .margin(egui::vec2(NAME_PADDING, 1.0)),
    );
    let name_label = tr(ui, "design-new-field");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, enabled, &name_label));
    if enabled && presentation.focus {
        response.request_focus();
    }
    // egui releases text focus on Escape before this response is inspected;
    // that submission frame still belongs to this exact pending name editor.
    if enabled
        && (response.has_focus() || response.lost_focus())
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        return Some(Event::Discard { revision });
    }
    if !enabled {
        response.on_disabled_hover_text(tr(ui, "datasheet-add-field-unavailable"));
    } else if !rejected && bounded_name(text.as_str()) && response.changed() {
        return Some(Event::Name(text.into_string()));
    }
    None
}

pub(crate) fn data_type(
    ui: &mut Ui,
    presentation: &Presentation,
    kind: Option<FieldAdditionKind>,
) -> Option<Event> {
    let selected = kind.map_or("", super::field_label_key);
    let mut event = None;
    let response = ui.add_enabled_ui(presentation.enabled, |ui| {
        egui::ComboBox::from_id_salt("saved_design_new_type")
            .width(ui.available_width())
            .selected_text(if selected.is_empty() {
                String::new()
            } else {
                tr(ui, selected)
            })
            .show_ui(ui, |ui| {
                for key in super::header::CHOICES {
                    let kind = super::header::kind_for_key(key);
                    let enabled = kind.is_some_and(|kind| presentation.choices.supports(kind));
                    let response = ui.add_enabled(enabled, egui::Button::new(tr(ui, key)));
                    if enabled && response.clicked() {
                        event = Some(Event::Choose(kind.unwrap()));
                        ui.close();
                    } else if !enabled {
                        response.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
                    }
                }
            })
            .response
    });
    let type_label = tr(ui, "design-data-type");
    response.inner.widget_info(|| {
        WidgetInfo::labeled(WidgetType::ComboBox, presentation.enabled, &type_label)
    });
    event
}

impl State {
    pub(in crate::workspace) fn apply_design_field_addition(
        &mut self,
        tab: &ObjectTab,
        event: Option<Event>,
    ) {
        if let Some(draft) = &mut self.field_addition.draft
            && draft.tab == *tab
        {
            draft.focus = false;
        }
        match event {
            Some(Event::Name(name)) if !self.field_addition_open() => {
                if let Some(declaration) = self
                    .field_addition_choices(tab)
                    .and_then(|choices| choices.default_declaration(FieldAdditionKind::ShortText))
                {
                    self.begin_field_addition_name(tab, name, declaration);
                }
            }
            Some(Event::Choose(kind)) if !self.field_addition_open() => {
                if let Some(declaration) = self
                    .field_addition_choices(tab)
                    .and_then(|choices| choices.default_declaration(kind))
                {
                    self.begin_field_addition_name(tab, String::new(), declaration);
                }
            }
            Some(Event::Choose(kind)) => {
                self.set_field_addition_kind(tab, kind);
            }
            event => self.apply_field_addition_header(tab, event),
        }
    }
}

#[cfg(test)]
mod tests;

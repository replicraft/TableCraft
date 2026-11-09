//! The first blank Design row is one staged proposal, never a native field.
use super::header::{Event, Presentation};
use crate::localization::tr;
use crate::workspace::field_name::{NameBuffer, bounded_name};
use crate::workspace::{ObjectTab, State};
use egui::{TextBuffer, TextEdit, Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::{
    FieldAdditionDeclaration, FieldAdditionKind, MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS,
};

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
    let enabled =
        presentation.enabled && (presentation.name.is_some() || presentation.choices.short_text);
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
    response.widget_info(|| {
        WidgetInfo::labeled(WidgetType::TextEdit, enabled, tr(ui, "design-new-field"))
    });
    if enabled && presentation.focus {
        response.request_focus();
    }
    if response.has_focus()
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
    let selected = kind.map_or("", |kind| match kind {
        FieldAdditionKind::Number => "table-draft-number",
        FieldAdditionKind::ShortText => "table-draft-short-text",
    });
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
                    let kind = match key {
                        "table-draft-number" => Some(FieldAdditionKind::Number),
                        "table-draft-short-text" => Some(FieldAdditionKind::ShortText),
                        _ => None,
                    };
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
    response.inner.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::ComboBox,
            presentation.enabled,
            tr(ui, "design-data-type"),
        )
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
                self.begin_field_addition_name(
                    tab,
                    name,
                    FieldAdditionDeclaration::ui_default(FieldAdditionKind::ShortText),
                );
            }
            Some(Event::Choose(kind)) if !self.field_addition_open() => {
                self.begin_field_addition_name(
                    tab,
                    String::new(),
                    FieldAdditionDeclaration::ui_default(kind),
                );
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

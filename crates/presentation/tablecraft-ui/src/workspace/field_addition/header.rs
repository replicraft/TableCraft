//! Saved-table trailing header. Each working choice needs its own positive source fact.
//!
//! The retained datasheet capture supplies the trailing-header placement. There
//! is no retained expanded-menu or saved-field naming capture; those controls
//! adapt the existing initial-table editor rather than establish visual parity.
use crate::localization::tr;
use crate::workspace::field_name::{NameBuffer, bounded_name};
use egui::TextBuffer;
use egui::{Button, RichText, TextEdit, Ui};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_builders::table::{MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

const MENU_WIDTH: f32 = 228.0;
const MAX_MENU_HEIGHT: f32 = 360.0;
const VIEWPORT_MARGIN: f32 = 24.0;
const NAME_PADDING: f32 = 2.0;

const CHOICES: [&str; 13] = [
    "table-draft-short-text",
    "table-draft-number",
    "table-draft-large-integer",
    "table-draft-currency",
    "table-draft-date-time",
    "table-draft-yes-no",
    "table-draft-long-text",
    "table-draft-lookup-relationship",
    "table-draft-rich-text",
    "table-draft-attachment",
    "table-draft-hyperlink",
    "table-draft-calculated-field",
    "table-draft-more-fields",
];

pub(crate) struct Presentation {
    pub(crate) enabled: bool,
    pub(crate) choices: super::FieldAdditionChoices,
    pub(crate) name: Option<(String, u64)>,
    pub(crate) focus: bool,
}

pub(crate) enum Event {
    Choose(FieldAdditionKind),
    Name(String),
    Discard { revision: u64 },
    MemoryLimit,
}

pub(crate) fn show(ui: &mut Ui, presentation: &Presentation) -> Option<Event> {
    if let Some((name, revision)) = &presentation.name {
        return name_editor(
            ui,
            name,
            *revision,
            presentation.enabled,
            presentation.focus,
        );
    }
    let mut event = None;
    let label = tr(ui, "datasheet-click-to-add");
    let response = ui.add_enabled_ui(presentation.enabled, |ui| {
        ui.menu_button(RichText::new(label).italics(), |ui| {
            ui.set_max_width(
                MENU_WIDTH.min((ui.ctx().content_rect().width() - VIEWPORT_MARGIN).max(100.0)),
            );
            egui::ScrollArea::vertical()
                .max_height(
                    MAX_MENU_HEIGHT
                        .min((ui.ctx().content_rect().height() - VIEWPORT_MARGIN).max(40.0)),
                )
                .show(ui, |ui| {
                    for key in CHOICES {
                        let kind = match key {
                            "table-draft-number" => Some(FieldAdditionKind::Number),
                            "table-draft-short-text" => Some(FieldAdditionKind::ShortText),
                            _ => None,
                        };
                        let enabled = kind.is_some_and(|kind| presentation.choices.supports(kind));
                        let response = ui.add_enabled(enabled, Button::new(tr(ui, key)));
                        if enabled && response.clicked() {
                            event = Some(Event::Choose(kind.unwrap()));
                            ui.close();
                        } else if !enabled {
                            response.on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
                        }
                    }
                });
        });
    });
    if !presentation.enabled {
        response
            .response
            .on_disabled_hover_text(tr(ui, "datasheet-add-field-unavailable"));
    }
    event
}

fn name_editor(
    ui: &mut Ui,
    original: &str,
    revision: u64,
    enabled: bool,
    focus: bool,
) -> Option<Event> {
    let Some(mut text) = NameBuffer::new(original) else {
        ui.add_enabled(false, egui::Label::new(original));
        return Some(Event::MemoryLimit);
    };
    let id = ui.make_persistent_id("saved_field_addition_name");
    let pointer_focus = ui.input(|input| input.events.iter().any(|event| {
        matches!(event, egui::Event::PointerButton { pos, pressed: true, .. } if ui.available_rect_before_wrap().contains(*pos))
    }));
    if enabled && (focus || pointer_focus) {
        ui.memory_mut(|memory| memory.request_focus(id));
    }
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
    if focus && enabled {
        response.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text.as_str().chars().count()),
                )));
            state.store(ui.ctx(), id);
        }
    }
    let label = tr(ui, "table-draft-field-name");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, enabled, &label));
    if !enabled {
        return None;
    }
    if response.has_focus()
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        return Some(Event::Discard { revision });
    }
    if !rejected && bounded_name(text.as_str()) && response.changed() && text.as_str() != original {
        return Some(Event::Name(text.into_string()));
    }
    None
}

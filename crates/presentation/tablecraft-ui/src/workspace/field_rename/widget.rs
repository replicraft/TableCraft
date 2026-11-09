//! Inline name adaptation beside the retained Datasheet/Design layout.
//! No retained Access capture shows this saved-field editor or its errors.
use super::FieldRenameTarget;
use crate::{localization::tr, workspace::field_name::NameBuffer};
use egui::{TextBuffer, TextEdit, Ui};
use tablecraft_builders::table::{MAX_DRAFT_NAME_BYTES, MAX_DRAFT_NAME_UNITS};

const HORIZONTAL_PADDING: f32 = 2.0;
const VERTICAL_PADDING: f32 = 1.0;

pub(crate) struct Presentation {
    pub target: FieldRenameTarget,
    pub proposed: String,
    pub revision: u64,
    pub enabled: bool,
    pub focus: bool,
}

pub(crate) enum Event {
    Begin(FieldRenameTarget),
    Name { revision: u64, name: String },
    Cancel { revision: u64 },
    MemoryLimit,
}

pub(crate) fn show(ui: &mut Ui, presentation: &Presentation) -> Option<Event> {
    let Some(mut text) = NameBuffer::new(&presentation.proposed) else {
        return Some(Event::MemoryLimit);
    };
    let id = ui.make_persistent_id((
        "saved_field_rename_name",
        presentation.target.tab.id,
        presentation.target.tab.target.document_revision,
        presentation.target.native_number,
        &presentation.target.column,
    ));
    let pointer_focus = ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(event, egui::Event::PointerButton { pos, pressed: true, .. }
                if ui.available_rect_before_wrap().contains(*pos))
        })
    });
    if presentation.enabled && (presentation.focus || pointer_focus) {
        ui.memory_mut(|memory| memory.request_focus(id));
    }

    let rejected = crate::text_input::reject_events_with_limits(
        ui,
        id,
        &presentation.proposed,
        MAX_DRAFT_NAME_BYTES,
        MAX_DRAFT_NAME_UNITS,
    );
    let response = ui.add_enabled(
        presentation.enabled,
        TextEdit::singleline(&mut text)
            .id(id)
            .desired_width(ui.available_width())
            .margin(egui::vec2(HORIZONTAL_PADDING, VERTICAL_PADDING)),
    );
    let name_label = tr(ui, "field-rename-name");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::TextEdit,
            presentation.enabled,
            &name_label,
        )
    });
    if presentation.focus && presentation.enabled {
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
    if !presentation.enabled {
        return None;
    }
    if response.has_focus()
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        return Some(Event::Cancel {
            revision: presentation.revision,
        });
    }
    if !rejected && response.changed() && text.as_str() != presentation.proposed {
        return Some(Event::Name {
            revision: presentation.revision,
            name: text.into_string(),
        });
    }
    None
}

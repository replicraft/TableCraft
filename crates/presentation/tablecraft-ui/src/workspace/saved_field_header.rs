//! One saved-field heading menu; filter/sort chevrons remain separate.
//! Delete surfaces have no retained Access capture and are adaptations.
use super::{
    DocumentView, FieldDropTarget, FieldRenameTarget, ObjectKind, ObjectTab, State,
    field_name::bounded_name, field_rename::widget,
};
use crate::{
    Column,
    localization::{message, tr},
};
use egui::{Key, Modifiers, Popup, Response, Sense, Ui};
use tablecraft_builders::table::MAX_DRAFT_NAME_BYTES;

pub(crate) enum Event {
    Rename(widget::Event),
    Drop(FieldDropTarget),
    MemoryLimit,
}

pub(crate) struct HeaderContext {
    pub tab: ObjectTab,
    pub rename_enabled: bool,
    pub drop_enabled: bool,
    pub editor: Option<widget::Presentation>,
    pub pending_drop: Option<FieldDropTarget>,
}

pub(crate) fn show(
    ui: &mut Ui,
    rect: egui::Rect,
    column: &Column,
    context: &HeaderContext,
    event: &mut Option<Event>,
) -> Response {
    if let Some(editor) = &context.editor
        && editor.target.column == column.name
        && column.native_number == Some(editor.target.native_number)
    {
        let response = ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| widget::show(ui, editor),
        );
        *event = response.inner.map(Event::Rename);
        return response.response;
    }
    let pending = context.pending_drop.as_ref().is_some_and(|target| {
        target.column == column.name && column.native_number == Some(target.native_number)
    });
    let label = if pending {
        egui::RichText::new(column.label()).strikethrough()
    } else {
        egui::RichText::new(column.label())
    };
    let mut response = ui
        .put(
            rect,
            egui::Label::new(label)
                .halign(egui::Align::Center)
                .truncate()
                .sense(Sense::click()),
        )
        .on_hover_text(&column.name);
    if pending {
        response = response.on_hover_text(message(
            ui,
            "field-drop-pending",
            &[("column", column.name.as_str().into())],
        ));
    }
    entry_menu(
        ui,
        &response,
        &context.tab,
        &column.name,
        column.native_number,
        context.rename_enabled,
        context.drop_enabled,
        event,
    );
    response
}

#[expect(
    clippy::too_many_arguments,
    reason = "Borrow actual header metadata and both distinct command gates without cloning."
)]
pub(crate) fn entry_menu(
    ui: &mut Ui,
    response: &Response,
    tab: &ObjectTab,
    name: &str,
    native_number: Option<usize>,
    rename_enabled: bool,
    drop_enabled: bool,
    event: &mut Option<Event>,
) {
    let rename_enabled = rename_enabled && native_number.is_some() && bounded_name(name);
    let drop_enabled = drop_enabled && native_number.is_some() && bounded_name(name);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        if rename_enabled || drop_enabled {
            node.add_action(egui::accesskit::Action::ShowContextMenu);
        }
    });
    if response.clicked() {
        response.request_focus();
    }
    let keyboard =
        response.has_focus() && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::F10));
    let accessible = ui.input_mut(|input| {
        let requested = input
            .has_accesskit_action_request(response.id, egui::accesskit::Action::ShowContextMenu);
        input.consume_accesskit_action_requests(response.id, |request| {
            request.action == egui::accesskit::Action::ShowContextMenu
        });
        requested
    });
    let popup_id = Popup::default_response_id(response);
    // Retain the existing rename popup identity/focus behavior.
    let origin_id = response.id.with("rename_keyboard_menu");
    let mut keyboard_origin = Popup::is_id_open(ui.ctx(), popup_id)
        && ui.data(|memory| memory.get_temp::<bool>(origin_id).unwrap_or(false));
    let mut rename = rename_enabled && response.double_clicked();
    let mut drop = false;
    let mut popup = Popup::context_menu(response);
    if keyboard || accessible {
        keyboard_origin = true;
        popup = popup.open_memory(egui::SetOpenCommand::Bool(true));
    } else if response.secondary_clicked() {
        keyboard_origin = false;
    }
    if keyboard_origin {
        popup = popup.anchor(response);
    }
    ui.data_mut(|memory| memory.insert_temp(origin_id, keyboard_origin));
    popup.show(|ui| {
        let rename_response = ui
            .add_enabled(
                rename_enabled,
                egui::Button::new(tr(ui, "field-rename-title")),
            )
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
        if (keyboard || accessible) && rename_enabled {
            rename_response.request_focus();
        }
        if rename_response.clicked() {
            rename = true;
            ui.close();
        }
        let drop_response = ui
            .add_enabled(drop_enabled, egui::Button::new(tr(ui, "field-drop-title")))
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
        if (keyboard || accessible) && !rename_enabled && drop_enabled {
            drop_response.request_focus();
        }
        if drop_response.clicked() {
            drop = true;
            ui.close();
        }
    });
    if rename && let Some(native_number) = native_number {
        *event = Some(
            match FieldRenameTarget::from_parts(tab, name, native_number) {
                Some(target) => Event::Rename(widget::Event::Begin(target)),
                None => Event::MemoryLimit,
            },
        );
    } else if drop && let Some(native_number) = native_number {
        *event = Some(
            match FieldDropTarget::from_parts(tab, name, native_number) {
                Some(target) => Event::Drop(target),
                None => Event::MemoryLimit,
            },
        );
    }
}

impl State {
    pub(in crate::workspace) fn saved_field_header_context(&mut self) -> Option<HeaderContext> {
        if self.view != DocumentView::Table {
            return None;
        }
        let tab = self.active_object()?;
        let ObjectKind::Table { name } = &tab.target.kind else {
            return None;
        };
        if !self.data.as_ref().is_some_and(|data| {
            data.source_revision == Some(tab.target.document_revision) && data.table == *name
        }) {
            return None;
        }
        let rename_enabled = self.field_rename_table_entry_ready(&tab);
        let drop_enabled = self.field_drop_table_entry_ready(&tab);
        let additional = name
            .capacity()
            .checked_add(4 * MAX_DRAFT_NAME_BYTES)?
            .checked_add(size_of::<HeaderContext>())?;
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }
        let editor = self.field_rename_editor_presentation();
        let pending_drop = self.field_drop_pending_target();
        Some(HeaderContext {
            tab,
            rename_enabled,
            drop_enabled,
            editor,
            pending_drop,
        })
    }

    pub(in crate::workspace) fn apply_saved_field_event(
        &mut self,
        event: Option<Event>,
        rename_target: Option<&FieldRenameTarget>,
        rename_enabled: bool,
    ) {
        match event {
            Some(Event::Rename(widget::Event::Begin(target))) => {
                self.begin_field_rename(target);
            }
            Some(Event::Rename(event)) => {
                if let Some(target) = rename_target {
                    self.apply_field_rename_widget(target, rename_enabled, Some(event));
                }
            }
            Some(Event::Drop(target)) => {
                self.begin_field_drop(target);
            }
            Some(Event::MemoryLimit) => {
                self.error = Some(self.localizer.text("objects-memory-limit"));
            }
            None => {
                if let Some(target) = rename_target {
                    self.apply_field_rename_widget(target, rename_enabled, None);
                }
            }
        }
    }
}

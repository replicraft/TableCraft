//! Saved Datasheet heading entry, separate from its filter/sort chevron.
use super::{FieldRenameTarget, rendering::HeaderContext, widget};
use crate::{Column, localization::tr, workspace::field_name::bounded_name};
use egui::{Key, Modifiers, Popup, Response, Sense, Ui};

pub(crate) fn show(
    ui: &mut Ui,
    rect: egui::Rect,
    column: &Column,
    context: &HeaderContext,
    event: &mut Option<widget::Event>,
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
        *event = response.inner;
        return response.response;
    }

    let response = ui
        .put(
            rect,
            egui::Label::new(column.label())
                .halign(egui::Align::Center)
                .truncate()
                .sense(Sense::click()),
        )
        .on_hover_text(&column.name);
    entry_menu(
        ui,
        &response,
        &context.tab,
        &column.name,
        column.native_number,
        context.commands_enabled,
        event,
    );
    response
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep borrowed field identity and popup ownership allocation-free."
)]
pub(crate) fn entry_menu(
    ui: &mut Ui,
    response: &Response,
    tab: &crate::ObjectTab,
    name: &str,
    native_number: Option<usize>,
    commands_enabled: bool,
    event: &mut Option<widget::Event>,
) {
    let enabled = commands_enabled && native_number.is_some() && bounded_name(name);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        if enabled {
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
    let origin_id = response.id.with("rename_keyboard_menu");
    let mut keyboard_origin = Popup::is_id_open(ui.ctx(), popup_id)
        && ui.data(|memory| memory.get_temp::<bool>(origin_id).unwrap_or(false));
    let mut begin = enabled && response.double_clicked();
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
        let rename = ui
            .add_enabled(enabled, egui::Button::new(tr(ui, "field-rename-title")))
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
        if keyboard || accessible {
            rename.request_focus();
        }
        if rename.clicked() {
            begin = true;
            ui.close();
        }
    });
    if begin {
        if let Some(native_number) = native_number {
            *event = Some(
                match FieldRenameTarget::from_parts(tab, name, native_number) {
                    Some(target) => widget::Event::Begin(target),
                    None => widget::Event::MemoryLimit,
                },
            );
        }
    }
}

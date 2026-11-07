//! Unsaved field headers keep native pointer and shortcut-menu distinctions.
use crate::localization::{message, tr};
use egui::{Key, Modifiers, Popup, Response, SetOpenCommand, Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::DraftField;

const FOCUS_STROKE_INSET: f32 = 1.0;

pub(super) enum Command {
    Rename,
    Delete,
}

pub(super) fn show(ui: &mut Ui, field: &DraftField, editable: bool) -> Option<Command> {
    let rect = ui.max_rect();
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("draft_field_header", field.id())),
        egui::Sense::click(),
    );
    let options = message(
        ui,
        "table-draft-field-options",
        &[("field", field.name().into())],
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), field.name()));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::ColumnHeader);
        node.set_description(options.clone());
        if ui.is_enabled() && editable {
            node.add_action(egui::accesskit::Action::ShowContextMenu);
        }
    });
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple(
        field.name().into(),
        font,
        ui.visuals().text_color(),
        rect.width(),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let text = ui.painter().layout_job(job);
    ui.painter().with_clip_rect(rect).galley(
        rect.center() - text.size() / 2.0,
        text,
        ui.visuals().text_color(),
    );
    if response.clicked() {
        response.request_focus();
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(FOCUS_STROKE_INSET),
            0.0,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let command = context_menu(ui, &response, editable);
    response.on_hover_text(options);
    command
}

fn context_menu(ui: &mut Ui, response: &Response, editable: bool) -> Option<Command> {
    let popup_id = Popup::default_response_id(response);
    let origin_id = response.id.with("field_context_keyboard");
    let was_open = Popup::is_id_open(ui.ctx(), popup_id);
    let mut keyboard_origin =
        was_open && ui.data(|memory| memory.get_temp::<bool>(origin_id).unwrap_or(false));
    if !ui.is_enabled() || !editable {
        Popup::close_id(ui.ctx(), popup_id);
        ui.data_mut(|memory| memory.remove::<bool>(origin_id));
        return None;
    }
    let requested =
        response.has_focus() && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::F10));
    let accessible = ui.input_mut(|input| {
        let requested = input
            .has_accesskit_action_request(response.id, egui::accesskit::Action::ShowContextMenu);
        input.consume_accesskit_action_requests(response.id, |request| {
            request.action == egui::accesskit::Action::ShowContextMenu
        });
        requested
    });
    let mut command = None;
    if response.double_clicked() {
        return Some(Command::Rename);
    }
    let mut popup = Popup::context_menu(response);
    if requested || accessible {
        keyboard_origin = true;
        popup = popup.open_memory(SetOpenCommand::Bool(true));
    } else if response.secondary_clicked() {
        keyboard_origin = false;
    }
    if keyboard_origin {
        popup = popup.anchor(response);
    }
    let restore_focus =
        keyboard_origin && was_open && ui.input(|input| input.key_pressed(Key::Escape));
    popup.show(|ui| {
        let rename = ui.button(tr(ui, "table-draft-rename-field"));
        if requested || accessible {
            rename.request_focus();
        }
        if rename.clicked() {
            command = Some(Command::Rename);
            ui.close();
        }
        if ui.button(tr(ui, "table-draft-delete-field")).clicked() {
            command = Some(Command::Delete);
            ui.close();
        }
    });
    let is_open = Popup::is_id_open(ui.ctx(), popup_id);
    ui.data_mut(|memory| {
        if is_open && keyboard_origin {
            memory.insert_temp(origin_id, true);
        } else {
            memory.remove::<bool>(origin_id);
        }
    });
    if restore_focus && !is_open {
        response.request_focus();
    }
    command
}

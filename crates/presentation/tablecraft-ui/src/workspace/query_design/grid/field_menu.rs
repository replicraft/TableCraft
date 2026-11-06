//! The existing field menu is reachable from its focused editor without a pointer.
use egui::{Key, Modifiers, Popup, Response, SetOpenCommand, Ui};

pub(super) fn show(ui: &mut Ui, response: &Response, contents: impl FnOnce(&mut Ui, bool)) {
    let popup_id = Popup::default_response_id(response);
    let origin_id = response.id.with("keyboard_field_menu");
    let was_open = Popup::is_id_open(ui.ctx(), popup_id);
    let mut keyboard_origin =
        was_open && ui.data(|memory| memory.get_temp::<bool>(origin_id).unwrap_or(false));
    let requested = response.enabled()
        && response.has_focus()
        && !Popup::is_any_open(ui.ctx())
        && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::F10));

    if !response.enabled() {
        Popup::close_id(ui.ctx(), popup_id);
        ui.data_mut(|memory| memory.remove::<bool>(origin_id));
        return;
    }
    if requested {
        keyboard_origin = true;
    } else if response.secondary_clicked() {
        keyboard_origin = false;
    }
    let restore_focus =
        keyboard_origin && was_open && ui.input(|input| input.key_pressed(Key::Escape));
    let mut popup = Popup::context_menu(response);
    if requested {
        popup = popup.open_memory(SetOpenCommand::Bool(true));
    }
    if keyboard_origin {
        popup = popup.anchor(response);
    }
    popup.show(|ui| contents(ui, requested));

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
}

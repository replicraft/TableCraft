//! Keyboard activation must preserve the Name dialog's actual focused control.
use super::{make_harness, state};
use crate::Action;
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::Queryable;

#[test]
fn tab_then_enter_activates_the_actual_ok_button_exactly_once() {
    let mut app = make_harness(state());
    let destination = app
        .get_by_role_and_label(Role::TextInput, "Query Name")
        .value()
        .unwrap();
    app.key_press(Key::Tab);
    app.run_steps(3);
    assert!(app.get_by_role_and_label(Role::Button, "OK").is_focused());
    app.key_press(Key::Enter);
    app.run_steps(2);
    let [Action::ReviewQueryCreation { request }] = app.state().1.as_slice() else {
        panic!("one actual focused-button submission expected");
    };
    assert_eq!(request.destination, destination);
    assert!(!app.state().0.query_creation_name_open());
    app.key_press(Key::Enter);
    app.run_steps(2);
    assert_eq!(app.state().1.len(), 1, "removed dialog cannot submit twice");
}

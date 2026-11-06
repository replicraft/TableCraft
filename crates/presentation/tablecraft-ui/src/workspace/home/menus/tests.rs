use crate::preview::{Preview, Scene};
use crate::workspace::{Action, State};
use egui::{Key, Popup, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn normal_home_menus_delegate_only_existing_select_refresh_and_readonly_zoom_commands() {
    let mut preview = Preview::new(Scene::Datasheet);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let original = harness.state().0.data.as_ref().unwrap().rows.as_ptr();

    harness
        .get_by_role_and_label(Role::Button, "Paste")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.paste_request.is_none());
    for label in ["Paste Special…", "Paste Append"] {
        let node = harness.get_by_role_and_label(Role::Button, label);
        assert!(node.accesskit_node().is_disabled());
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Focus)
        );
        node.focus();
        node.click_accesskit();
    }
    harness.run_steps(3);
    assert!(harness.state().0.paste_request.is_none());
    assert!(harness.state().1.is_empty());
    {
        let state = &mut harness.state_mut().0;
        state
            .table_selection
            .select(state.data.as_ref().unwrap(), 1, 0);
    }
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.paste_request.is_none());
    let paste_target = harness.state().0.current_paste_target().unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Paste")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Paste text")
        .click_accesskit();
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.paste_request.is_some());
    harness.event(egui::Event::Paste("Literal NULL\nnext\tfield".into()));
    harness.run_steps(1);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::PasteCell { target, text }] if target == &paste_target && text == "Literal NULL\nnext\tfield"
    ));
    assert!(!harness.state().0.edit_dialog_open());
    harness.state_mut().1.clear();

    harness
        .get_by_role_and_label(Role::Button, "Zoom")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    assert!(!harness.state().0.zoom_dialog_open());
    harness
        .get_all_by_role_and_label(Role::Button, "Zoom")
        .next_back()
        .unwrap()
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.zoom_dialog_open());
    assert!(harness.state().1.is_empty());
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(!harness.state().0.zoom_dialog_open());

    harness
        .get_by_role_and_label(Role::Button, "Refresh All")
        .click_accesskit();
    harness.run_steps(3);
    let unavailable = harness
        .get_all_by_role_and_label(Role::Button, "Refresh All")
        .next_back()
        .unwrap();
    assert!(unavailable.accesskit_node().is_disabled());
    assert!(
        !unavailable
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Click)
    );
    harness
        .get_by_role_and_label(Role::Button, "Refresh")
        .click_accesskit();
    harness.run_steps(3);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::Page { table, offset: 0 }] if table == "Customers")
    );
    harness.state_mut().1.clear();

    harness
        .get_by_role_and_label(Role::Button, "Select")
        .click_accesskit();
    harness.run_steps(3);
    let all = harness.get_by_role_and_label(Role::Button, "Select All");
    assert!(all.accesskit_node().is_disabled());
    assert!(
        !all.accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Click)
    );
    assert!(
        !all.accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus)
    );
    harness
        .get_by_role_and_label(Role::Button, "Select record")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.record_selected());
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().rows.as_ptr(),
        original
    );
    assert!(!Popup::is_any_open(&harness.ctx));

    let delete_target = harness.state().0.current_delete_target().unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.deletion_dialog_open());
    harness
        .get_by_role_and_label(Role::Button, "Delete row")
        .click_accesskit();
    harness.run_steps(3);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::ReviewDelete { target }] if target == &delete_target
    ));
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    {
        let state = &mut harness.state_mut().0;
        let selected = state.table_selection.position().unwrap();
        let other_row = if selected.0 == 0 { 1 } else { 0 };
        state
            .table_selection
            .select(state.data.as_ref().unwrap(), other_row, selected.1);
        assert_ne!(state.current_delete_target().as_ref(), Some(&delete_target));
    }
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete row")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    harness.state_mut().0.busy = true;
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete row")
            .is_none()
    );
    for label in ["Select", "Refresh All", "Zoom", "Delete"] {
        assert!(
            harness
                .get_by_role_and_label(Role::Button, label)
                .accesskit_node()
                .is_disabled()
        );
    }
    assert!(harness.state().1.is_empty());
}

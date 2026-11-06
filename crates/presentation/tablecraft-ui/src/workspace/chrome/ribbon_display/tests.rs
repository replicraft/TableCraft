use crate::preview::{Preview, Scene};
use crate::workspace::{
    Action, State,
    objects::{ObjectKind, ObjectTarget},
};
use egui::{Key, Modifiers, Popup, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};

fn workspace(scene: Scene) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(scene);
    let state = std::mem::take(preview.state_mut());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

#[test]
fn shortcut_and_actual_tab_double_click_toggle_only_ribbon_presentation() {
    let mut harness = workspace(Scene::Datasheet);
    let rows = harness.state().0.data.as_ref().unwrap().rows.as_ptr();
    let original_top = harness
        .get_by_role_and_label(Role::Tab, "Customers")
        .rect()
        .top();

    harness.key_press_modifiers(Modifiers::CTRL, Key::F1);
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_collapsed());
    assert!(!harness.state().0.ribbon_temporarily_open());
    let collapsed_top = harness
        .get_by_role_and_label(Role::Tab, "Customers")
        .rect()
        .top();
    assert!(collapsed_top < original_top - 40.0);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Refresh All")
            .is_none()
    );

    harness.key_press_modifiers(Modifiers::CTRL, Key::F1);
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_collapsed());
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(3);
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_collapsed());

    harness.run_steps(60);
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(3);
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_collapsed());
    assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(harness.state().1.is_empty());
}

#[test]
fn temporary_ribbon_preserves_layout_and_child_menu_escape_before_dismissal() {
    let mut harness = workspace(Scene::Datasheet);
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    let collapsed_top = harness
        .get_by_role_and_label(Role::Tab, "Customers")
        .rect()
        .top();
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_temporarily_open());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::Tab, "Customers")
            .rect()
            .top(),
        collapsed_top
    );

    harness
        .get_by_role_and_label(Role::Button, "Select")
        .click_accesskit();
    harness.run_steps(3);
    assert!(Popup::is_any_open(&harness.ctx));
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.ribbon_temporarily_open());
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(harness.state().0.ribbon_collapsed());
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Pin the ribbon")
        .click_accesskit();
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_collapsed());
    assert!(!harness.state().0.ribbon_temporarily_open());
}

#[test]
fn temporary_ribbon_dismisses_after_real_command_and_backstage_and_ignores_modal_shortcut() {
    let mut harness = workspace(Scene::Datasheet);
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Refresh All")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Refresh")
        .click_accesskit();
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(
        matches!(harness.state().1.as_slice(), [Action::Page { table, offset: 0 }] if table == "Customers")
    );
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness.state_mut().0.show_open();
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    harness.key_press_modifiers(Modifiers::CTRL, Key::F1);
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_collapsed());

    let mut modal = workspace(Scene::Zoom);
    assert!(modal.state().0.zoom_dialog_open());
    modal.key_press_modifiers(Modifiers::CTRL, Key::F1);
    modal.run_steps(3);
    assert!(!modal.state().0.ribbon_collapsed());
    assert!(modal.state().0.zoom_dialog_open());
    assert!(modal.state().1.is_empty());
}

#[test]
fn outside_record_click_and_f6_dismiss_peek_without_losing_document_selection() {
    let mut harness = workspace(Scene::Datasheet);
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_temporarily_open());

    let record = harness
        .get_all_by_role(Role::RowHeader)
        .find(|record| record.rect().top() > 400.0)
        .expect("a native record selector below the temporary command overlay");
    record.click();
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(harness.state().0.record_selected());
    let selection = harness.state().0.table_selection.position().unwrap();
    assert!(selection.0 > 0);

    harness.get_by_role_and_label(Role::Tab, "Home").focus();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_temporarily_open());
    harness.key_press(Key::F6);
    harness.run_steps(4);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert_eq!(
        harness.state().0.table_selection.position(),
        Some(selection)
    );
    assert!(harness.state().0.record_selected());
    assert!(harness.state().1.is_empty());
}

#[test]
fn opening_a_different_real_object_drops_temporary_ribbon_scope() {
    let mut harness = workspace(Scene::Datasheet);
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.ribbon_temporarily_open());

    let revision = harness
        .state()
        .0
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert!(
        harness
            .state_mut()
            .0
            .prepare_object_open(ObjectTarget {
                document_revision: revision,
                kind: ObjectKind::Sql,
            })
            .is_some()
    );
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(harness.state().0.ribbon_collapsed());
    assert_eq!(
        harness.state().0.active_object().unwrap().target.kind,
        ObjectKind::Sql
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Refresh All")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn collapse_control_then_same_tab_pair_expands_without_an_idle_gap() {
    let mut harness = workspace(Scene::Datasheet);
    let rows = harness.state().0.data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::Button, "Collapse the ribbon (Ctrl+F1)")
        .click();
    harness.run_steps(1);
    assert!(harness.state().0.ribbon_collapsed());
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(1);
    assert!(harness.state().0.ribbon_temporarily_open());
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(1);
    assert!(
        !harness.state().0.ribbon_collapsed(),
        "two clicks on Home form a tab-owned pair after the separate collapse control"
    );
    harness.get_by_role_and_label(Role::Tab, "Home").click();
    harness.run_steps(1);
    assert!(!harness.state().0.ribbon_collapsed());
    assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(harness.state().1.is_empty());
}

#[test]
fn temporary_font_catalog_owns_text_and_f6_and_dismisses_without_document_actions() {
    use egui_kittest::kittest::NodeT;

    let mut harness = workspace(Scene::Datasheet);
    harness.state_mut().0.font_families =
        crate::FontFamilies::from_system(["Example Serif", "Verdana"]).unwrap();
    let rows = harness.state().0.data.as_ref().unwrap().rows.as_ptr();
    let selection = harness.state().0.table_selection.position();
    let sql = harness.state().0.sql.clone();
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    assert!(
        harness
            .query_by_role_and_label(Role::ComboBox, "Font")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::ComboBox, "Font")
        .click_accesskit();
    harness.run_steps(3);
    let search = harness.get_by_role_and_label(Role::TextInput, "Search fonts");
    assert!(search.accesskit_node().is_focused());
    harness.event(egui::Event::Text("Verdana".into()));
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Verdana")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Example Serif")
            .is_none()
    );
    harness.key_press(Key::F6);
    harness.run_steps(3);
    let search = harness.get_by_role_and_label(Role::TextInput, "Search fonts");
    assert!(search.accesskit_node().is_focused());
    assert_eq!(search.value().as_deref(), Some("Verdana"));
    assert!(harness.state().0.ribbon_temporarily_open());
    assert_eq!(harness.state().0.sql, sql);
    assert_eq!(harness.state().0.table_selection.position(), selection);

    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Paste("sElA".into()));
    harness.run_steps(3);
    let search = harness.get_by_role_and_label(Role::TextInput, "Search fonts");
    assert!(search.accesskit_node().is_focused());
    assert_eq!(search.value().as_deref(), Some("sElA"));
    harness.get_by_role_and_label(Role::Button, "Selawik");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Verdana")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.edit_dialog_open());
    assert_eq!(harness.state().0.sql, sql);
    assert_eq!(harness.state().0.table_selection.position(), selection);

    harness.state_mut().0.busy = true;
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.ribbon_temporarily_open());
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.busy);
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(harness.state().0.ribbon_collapsed());
    assert!(harness.state().0.busy);
    assert!(harness.state().1.is_empty());
    harness.state_mut().0.busy = false;

    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::ComboBox, "Font")
        .click_accesskit();
    harness.run_steps(3);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Search fonts")
            .value()
            .as_deref(),
        Some("")
    );
    let outside = harness
        .get_all_by_role(Role::RowHeader)
        .nth(20)
        .unwrap()
        .rect()
        .center();
    harness.event(egui::Event::PointerMoved(outside));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::ComboBox, "Font")
        .click_accesskit();
    harness.run_steps(3);
    let revision = harness
        .state()
        .0
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert!(
        harness
            .state_mut()
            .0
            .prepare_object_open(ObjectTarget {
                document_revision: revision,
                kind: ObjectKind::Sql,
            })
            .is_some()
    );
    harness.run_steps(3);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Search fonts")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
}

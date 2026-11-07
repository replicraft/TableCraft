use super::*;
use crate::preview::{Preview, Scene};
use crate::{Action, Cell, Data, Locale, Localizer};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::mem;

fn workspace(scene: Scene, locale: Locale, width: f32) -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(scene);
    let mut state = mem::take(preview.state_mut());
    state.localizer = Localizer::new(locale).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(width, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn press(harness: &mut Harness<'_, (State, Vec<Action>)>, key: Key) {
    harness.key_press(key);
    harness.run_steps(3);
}

#[test]
fn f10_tabs_groups_and_commands_navigate_real_widgets_without_running_actions() {
    let mut harness = workspace(Scene::Datasheet, Locale::EnUs, 1600.0);
    let rows = harness.state().0.data.as_ref().unwrap().rows.as_ptr();
    let selection = harness.state().0.table_selection.position();
    press(&mut harness, Key::F10);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::ArrowRight);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Create")
            .accesskit_node()
            .is_focused()
    );
    assert!(harness.state().0.ribbon_tab == Tab::Create);
    press(&mut harness, Key::ArrowLeft);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::ArrowDown);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "View")
            .accesskit_node()
            .is_focused()
    );
    harness.key_press_modifiers(Modifiers::CTRL, Key::ArrowRight);
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Paste")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::Tab);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Copy")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::Tab);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Filter")
            .accesskit_node()
            .is_focused()
    );
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Copy")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::Escape);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::Escape);
    assert!(
        !harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    assert!(
        harness
            .get_all_by_role(Role::Label)
            .any(|node| node.accesskit_node().is_focused())
    );
    assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert_eq!(harness.state().0.table_selection.position(), selection);
    assert!(harness.state().1.is_empty());
}

#[test]
fn compact_translated_ribbons_reveal_the_actual_focused_font_control() {
    for locale in [Locale::De, Locale::Ja] {
        let mut harness = workspace(Scene::Datasheet, locale, 900.0);
        let font = harness.state().0.localizer.text("ribbon-font");
        let home = harness.state().0.localizer.text("ribbon-home");
        press(&mut harness, Key::F10);
        assert!(
            harness
                .get_by_role_and_label(Role::Tab, &home)
                .accesskit_node()
                .is_focused()
        );
        press(&mut harness, Key::ArrowDown);
        for _ in 0..10 {
            if harness
                .get_by_role_and_label(Role::ComboBox, &font)
                .accesskit_node()
                .is_focused()
            {
                break;
            }
            harness.key_press_modifiers(Modifiers::CTRL, Key::ArrowRight);
            harness.run_steps(3);
        }
        let combo = harness.get_by_role_and_label(Role::ComboBox, &font);
        assert!(combo.accesskit_node().is_focused());
        let viewport = harness.ctx.content_rect();
        assert!(
            viewport.contains_rect(combo.rect()),
            "focused translated control must be revealed: control {:?}, viewport {:?}",
            combo.rect(),
            viewport
        );
        assert!(combo.rect().width() > 50.0 && combo.rect().height() > 10.0);
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn collapsed_entry_respects_font_popup_and_a_real_find_dialog_focus_owner() {
    let mut harness = workspace(Scene::Datasheet, Locale::EnUs, 1600.0);
    harness.state_mut().0.set_ribbon_collapsed(true);
    harness.run_steps(3);
    press(&mut harness, Key::F10);
    press(&mut harness, Key::ArrowDown);
    assert!(harness.state().0.ribbon_temporarily_open());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "View")
            .accesskit_node()
            .is_focused()
    );
    harness
        .get_by_role_and_label(Role::ComboBox, "Font")
        .click_accesskit();
    harness.run_steps(3);
    harness.event(Event::Text("sela".into()));
    harness.key_press(Key::F10);
    harness.key_press(Key::F6);
    harness.run_steps(3);
    let search = harness.get_by_role_and_label(Role::TextInput, "Search fonts");
    assert!(search.accesskit_node().is_focused());
    assert_eq!(search.accesskit_node().value().as_deref(), Some("sela"));
    press(&mut harness, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(harness.state().0.ribbon_temporarily_open());
    press(&mut harness, Key::Escape);
    assert!(!harness.state().0.ribbon_temporarily_open());
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );

    press(&mut harness, Key::ArrowDown);
    harness.get_by_role_and_label(Role::Button, "Find").focus();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Find")
        .click_accesskit();
    harness.key_press(Key::Tab);
    harness.run_steps(3);
    assert!(harness.state().0.find_window_open());
    assert!(!harness.state().0.ribbon_temporarily_open());
    // The queued Tab belongs to the newly opened Find window and advances
    // from Find What to its next control; Ribbon navigation must not steal it.
    assert!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Look In:")
            .accesskit_node()
            .is_focused()
    );
    press(&mut harness, Key::F10);
    assert!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Look In:")
            .accesskit_node()
            .is_focused()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn f10_keeps_text_and_ime_owners_and_exact_shortcut_modifiers() {
    let mut harness = workspace(Scene::Query, Locale::EnUs, 1600.0);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(3);
    let sql = harness.state().0.sql.clone();
    for modifiers in [Modifiers::SHIFT, Modifiers::CTRL, Modifiers::ALT] {
        harness.key_press_modifiers(modifiers, Key::F10);
        harness.run_steps(3);
        assert!(
            harness
                .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
                .accesskit_node()
                .is_focused()
        );
    }
    harness.event(Event::Ime(ImeEvent::Preedit {
        text: "あ".into(),
        active_range_chars: None,
    }));
    harness.run_steps(1);
    press(&mut harness, Key::F10);
    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .accesskit_node()
            .is_focused()
    );
    harness.event(Event::Ime(ImeEvent::Preedit {
        text: String::new(),
        active_range_chars: None,
    }));
    harness.run_steps(3);
    press(&mut harness, Key::F10);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    assert_eq!(harness.state().0.sql, sql);
    assert!(harness.state().1.is_empty());
}

#[test]
fn deferred_command_entry_cannot_cross_a_new_publication_tab_or_document() {
    for change in ["publication", "tab", "document"] {
        let mut harness = workspace(Scene::Datasheet, Locale::EnUs, 1600.0);
        press(&mut harness, Key::F10);
        let focused = harness.ctx.memory(|memory| memory.focused());
        let scope = harness.state().0.ribbon_keyboard_scope();
        harness.state_mut().0.ribbon_keyboard.pending = Some(Entry {
            scope,
            focus_witness: focused,
            display: (false, false),
        });
        match change {
            "publication" => {
                let revision = harness
                    .state()
                    .0
                    .database
                    .as_ref()
                    .unwrap()
                    .document_revision;
                harness.state_mut().0.set_table_data(Data {
                    table: "Customers".into(),
                    source_revision: Some(revision),
                    row_ordinals: None,
                    offset: 100,
                    columns: vec!["CustomerID".into()],
                    rows: vec![vec![Cell::scalar("101")]],
                    has_more: false,
                });
            }
            "tab" => harness.state_mut().0.ribbon_tab = Tab::Create,
            "document" => {
                harness
                    .state_mut()
                    .0
                    .database
                    .as_mut()
                    .unwrap()
                    .document_revision += 1
            }
            _ => unreachable!(),
        }
        harness.run_steps(3);
        assert!(harness.state().0.ribbon_keyboard.pending.is_none());
        assert!(
            !harness
                .state()
                .0
                .ribbon_keyboard
                .witness
                .as_ref()
                .is_some_and(|witness| witness.commands)
        );
        assert!(harness.state().1.is_empty());
    }
}

#[test]
#[allow(deprecated)]
fn ime_teardown_without_commit_releases_the_same_editor_for_explicit_f10() {
    let mut harness = workspace(Scene::Query, Locale::EnUs, 1600.0);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(3);
    harness.event(Event::Ime(ImeEvent::Preedit {
        text: "あ".into(),
        active_range_chars: None,
    }));
    harness.run_steps(1);
    assert!(harness.state().0.ribbon_keyboard.composition.is_some());
    harness.event(Event::Ime(ImeEvent::Disabled));
    harness.run_steps(1);
    let sql = harness.state().0.sql.clone();
    press(&mut harness, Key::F10);
    assert!(
        harness
            .get_by_role_and_label(Role::Tab, "Home")
            .accesskit_node()
            .is_focused()
    );
    assert_eq!(harness.state().0.sql, sql);
    assert!(harness.state().1.is_empty());
}

#[test]
fn f10_revalidates_after_a_real_sql_object_open_removes_the_contextual_table_tab() {
    let mut harness = workspace(Scene::Datasheet, Locale::EnUs, 1600.0);
    let table = harness.state().0.active_object().unwrap();
    let rows = harness.state().0.data.as_ref().unwrap().rows.as_ptr();
    let selection = harness.state().0.table_selection.position();
    harness
        .get_by_role_and_label(Role::Tab, "Table Fields")
        .click_accesskit();
    harness.run_steps(3);
    press(&mut harness, Key::F10);
    let contextual = harness.get_by_role_and_label(Role::Tab, "Table Fields");
    assert!(contextual.accesskit_node().is_focused());
    let contextual_id = contextual.accesskit_node().id();
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(3);
    let actions = mem::take(&mut harness.state_mut().1);
    let [Action::OpenObject { target }] = actions.as_slice() else {
        panic!("opening the scratch object must issue only its inert object request");
    };
    assert!(matches!(target.kind, crate::ObjectKind::Sql));
    let opened = harness
        .state_mut()
        .0
        .prepare_object_open(target.clone())
        .unwrap();
    assert!(!opened.needs_load);
    harness.run_steps(3);
    assert!(
        harness
            .query_by_role_and_label(Role::Tab, "Table Fields")
            .is_none()
    );
    press(&mut harness, Key::F10);
    let home = harness.get_by_role_and_label(Role::Tab, "Home");
    assert!(home.accesskit_node().is_focused());
    assert_ne!(home.accesskit_node().id(), contextual_id);
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Tab, "Customers")
        .click_accesskit();
    harness.run_steps(3);
    let actions = mem::take(&mut harness.state_mut().1);
    let [Action::ActivateObject { tab }] = actions.as_slice() else {
        panic!("returning to the retained table must issue only its activation request");
    };
    assert_eq!(tab, &table);
    assert!(harness.state_mut().0.activate_object(tab));
    harness.run_steps(3);
    assert_eq!(harness.state().0.active_object(), Some(table));
    assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert_eq!(harness.state().0.table_selection.position(), selection);
    assert!(harness.state().1.is_empty());
}

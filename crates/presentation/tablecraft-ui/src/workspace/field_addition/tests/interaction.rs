//! Real header widgets emit only an exact naming review, never a native write.
use crate::{Action, Data, Database, ObjectKind, ObjectTarget, State, Table};
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};

pub(in crate::workspace::field_addition) fn state(available: bool) -> State {
    let mut state = State {
        field_addition_available: available,
        database: Some(Database {
            document_revision: 7,
            title_path: None,
            title: "Field naming".into(),
            format: "ACE 16".into(),
            queries: Vec::new(),
            tables: vec![Table {
                name: "Table1".into(),
                local: true,
                system: false,
                storage: "Local".into(),
            }],
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state
        .prepare_object_open(ObjectTarget {
            document_revision: 7,
            kind: ObjectKind::Table {
                name: "Table1".into(),
            },
        })
        .unwrap();
    assert!(state.set_table_data(Data {
        source_revision: Some(7),
        row_ordinals: None,
        table: "Table1".into(),
        offset: 0,
        columns: vec!["ID".into()],
        rows: Vec::new(),
        has_more: false,
    }));
    state
}

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 720.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

#[test]
fn host_support_and_positive_source_fact_are_independent_header_gates() {
    for supported in [false, true] {
        let mut harness = harness(state(supported));
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Click to Add")
                .accesskit_node()
                .is_disabled()
        );
        assert!(harness.state().1.is_empty());
        let tab = harness.state().0.active_object().unwrap();
        if supported {
            assert!(
                harness
                    .state_mut()
                    .0
                    .begin_field_addition_inspection(&tab, 80)
                    .is_ok()
            );
            assert!(
                harness
                    .state_mut()
                    .0
                    .set_field_addition_availability(&tab, 80, false, false)
            );
            harness.run_steps(2);
            assert!(
                harness
                    .get_by_role_and_label(Role::Button, "Click to Add")
                    .accesskit_node()
                    .is_disabled()
            );
        } else {
            assert!(
                harness
                    .state_mut()
                    .0
                    .begin_field_addition_inspection(&tab, 80)
                    .is_err()
            );
        }
    }
}

#[test]
fn saved_number_header_keeps_stored_schema_inert_and_ctrl_s_captures_final_text() {
    header_review(FieldAdditionKind::Number, "Number", "Score");
}

#[test]
fn saved_short_text_header_uses_the_same_editor_and_captures_its_full_declaration() {
    header_review(FieldAdditionKind::ShortText, "Short Text", "Label");
}

fn header_review(kind: FieldAdditionKind, label: &str, name: &str) {
    let mut state = state(true);
    let tab = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
    assert!(state.set_field_addition_availability(
        &tab,
        80,
        kind == FieldAdditionKind::Number,
        kind == FieldAdditionKind::ShortText
    ));
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, label)
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Currency")
            .accesskit_node()
            .is_disabled()
    );
    let unavailable = if kind == FieldAdditionKind::Number {
        "Short Text"
    } else {
        "Number"
    };
    assert!(
        harness
            .get_by_role_and_label(Role::Button, unavailable)
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, label)
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().0.field_addition_name().unwrap().1, "Field1");
    assert_eq!(harness.state().0.data.as_ref().unwrap().columns.len(), 1);
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(name);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let [Action::ReviewFieldAddition { request }] = harness.state().1.as_slice() else {
        panic!("exactly one named review expected");
    };
    assert_eq!(request.tab, tab);
    assert_eq!(request.column, name);
    assert_eq!(
        request.declaration,
        FieldAdditionDeclaration::ui_default(kind)
    );
    assert_eq!(request.draft_revision, 1);
    assert!(harness.state().0.field_addition_request_is_current(request));
    assert_eq!(harness.state().0.data.as_ref().unwrap().columns.len(), 1);
}

#[test]
fn terminal_source_observation_survives_busy_or_changed_view_context() {
    let mut state = state(true);
    let tab = state.active_object().unwrap();
    assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
    state.busy = true;
    assert!(state.set_field_addition_availability(&tab, 80, true, false));
    assert!(!state.can_name_field_addition(&tab, FieldAdditionKind::Number));
    state.busy = false;
    assert!(state.can_name_field_addition(&tab, FieldAdditionKind::Number));
}

#[test]
fn actual_name_widget_refuses_astral_paste_and_ime_before_draft_or_undo_can_own_it() {
    for event in [
        egui::Event::Paste("😀".repeat(33)),
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "😀".repeat(33),
            active_range_chars: None,
        }),
        egui::Event::Ime(egui::ImeEvent::Commit("😀".repeat(33))),
    ] {
        let mut state = state(true);
        let tab = state.active_object().unwrap();
        assert!(state.begin_field_addition_inspection(&tab, 80).is_ok());
        assert!(state.set_field_addition_availability(&tab, 80, true, false));
        assert!(state.begin_default_field_addition(&tab, FieldAdditionKind::Number));
        let mut harness = harness(state);
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .focus();
        harness.input_mut().events.push(event);
        harness.run_steps(1);
        assert_eq!(harness.state().0.field_addition_name().unwrap().1, "Field1");
        assert_eq!(harness.state().0.field_addition_name().unwrap().2, 0);
        assert!(harness.state().1.is_empty());
    }
}

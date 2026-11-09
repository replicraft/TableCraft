//! Actual widgets over source-free UI facts, not native write qualification.
use super::{confirming, observed, pending, saving};
use crate::{
    Action, EditSaveOutcome, Field, FieldDropSaveCommand, Locale, Localizer, Schema, State,
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

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

fn unopened() -> State {
    let (mut state, _) = confirming();
    state.dismiss_field_drop_confirmation().unwrap();
    state
}

fn design() -> State {
    let mut state = unopened();
    let tab = state.active_object().unwrap();
    assert!(state.prepare_object_design(&tab).unwrap().needs_load);
    state.set_schema(Schema {
        table: "Table1".into(),
        declared_rows: 0,
        fields: vec![Field {
            name: "Score".into(),
            number: 17,
            native_type: 4,
            storage_bytes: 4,
            fixed: true,
            flags: 3,
            extended_flags: 0,
            scalar_type: Some("Int32".into()),
            display_type: Some("Number".into()),
            field_size: Some("Long Integer".into()),
            reader_nullable: Some(true),
            decimal_parameters: None,
            complex_type_id: None,
            properties: Vec::new(),
        }],
    });
    state
}

#[test]
fn captioned_datasheet_menu_inspects_true_field_without_a_review_or_save() {
    let mut harness = harness(unopened());
    harness.get_by_label("Friendly").click();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete Field")
        .click_accesskit();
    harness.run_steps(1);
    let [Action::InspectFieldDrop { target }] = harness.state().1.as_slice() else {
        panic!("exactly one readonly DROP inspection expected");
    };
    assert_eq!(target.column, "Score");
    assert_eq!(target.native_number, 17);
    assert_eq!(target.tab, harness.state().0.active_object().unwrap());
    assert!(!harness.state().0.field_drop_dirty());
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().columns[1].name,
        "Score"
    );
}

#[test]
fn unavailable_delete_menu_stays_visible_disabled_and_never_dispatches() {
    let mut state = unopened();
    state.field_drop_available = false;
    let mut harness = harness(state);
    harness.get_by_label("Friendly").click();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    let button = harness.get_by_role_and_label(Role::Button, "Delete Field");
    assert!(button.accesskit_node().is_disabled());
    button.click();
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.field_drop_open());
}

#[test]
fn saved_design_delete_rows_and_delete_key_bind_actual_native_field() {
    for keyboard in [false, true] {
        let mut harness = harness(design());
        if keyboard {
            harness.get_by_label("Score").focus();
            harness.run_steps(1);
            harness.key_press(Key::Delete);
        } else {
            harness
                .get_by_role_and_label(Role::Button, "Delete Rows")
                .click_accesskit();
        }
        harness.run_steps(1);
        let [Action::InspectFieldDrop { target }] = harness.state().1.as_slice() else {
            panic!("Design removal starts only readonly inspection");
        };
        assert_eq!(target.column, "Score");
        assert_eq!(target.native_number, 17);
        let field = &harness.state().0.schema.as_ref().unwrap().fields[0];
        assert_eq!(field.name, "Score");
        assert_eq!(field.number, 17);
        assert!(!harness.state().0.table_draft_design_visible());
    }
}

#[test]
fn no_is_default_and_escape_revokes_only_the_actual_readonly_request() {
    let (mut state, target) = confirming();
    state
        .prepare_field_drop_inspection(&target)
        .unwrap()
        .bind(80);
    let mut harness = harness(state);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "No")
            .is_focused()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(1);
    assert!(!harness.state().0.field_drop_open());
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CancelFieldDropInspection { request_id: 80 }]
    ));
    assert_eq!(harness.state().0.data.as_ref().unwrap().columns.len(), 2);
}

#[test]
fn yes_only_marks_pending_then_explicit_save_or_ctrl_s_captures_exact_request() {
    for shortcut in [false, true] {
        let (state, target) = observed();
        let mut harness = harness(state);
        harness
            .get_by_role_and_label(Role::Button, "Yes")
            .click_accesskit();
        harness.run_steps(2);
        assert!(harness.state().1.is_empty());
        assert!(harness.state().0.field_drop_dirty());
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().columns[1].name,
            "Score"
        );
        let pending = harness.state().0.localizer.format("field-drop-pending", &{
            let mut args = crate::FluentArgs::new();
            args.set("column", "Score");
            args
        });
        harness.get_by_label(&pending);
        if shortcut {
            harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        } else {
            let save = harness
                .get_all_by_role_and_label(Role::Button, "Save")
                .max_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
                .expect("actual pending-deletion Save footer");
            save.click_accesskit();
        }
        harness.run_steps(1);
        let [Action::ReviewFieldDrop { request }] = harness.state().1.as_slice() else {
            panic!("exactly one explicit full DROP review expected");
        };
        assert_eq!(request.target, target);
        assert_eq!(request.draft_revision, 1);
        assert_eq!(harness.state().0.data.as_ref().unwrap().columns.len(), 2);
    }
}

#[test]
fn dirty_close_buttons_return_exact_save_discard_cancel_choices() {
    for decision in [
        crate::FieldDropCloseDecision::Save,
        crate::FieldDropCloseDecision::Discard,
        crate::FieldDropCloseDecision::Cancel,
    ] {
        let (mut state, request) = pending();
        assert!(state.request_current_field_drop_close());
        let mut harness = harness(state);
        let label = match decision {
            crate::FieldDropCloseDecision::Save => "Save",
            crate::FieldDropCloseDecision::Discard => "Discard",
            crate::FieldDropCloseDecision::Cancel => "Cancel",
        };
        harness
            .get_by_role_and_label(Role::Button, label)
            .click_accesskit();
        harness.run_steps(1);
        let [Action::FieldDropClose { result }] = harness.state().1.as_slice() else {
            panic!("exact close result expected");
        };
        assert_eq!(result.target, request.target);
        assert_eq!(result.draft_revision, request.draft_revision);
        assert_eq!(result.decision, decision);
        assert_eq!(
            harness.state().0.field_drop_open(),
            decision != crate::FieldDropCloseDecision::Discard
        );
        assert_eq!(harness.state().0.data.as_ref().unwrap().columns.len(), 2);
    }
}

#[test]
fn retained_status_and_reopening_escape_emit_only_exact_actor_commands() {
    let (mut state, request, identity) = saving();
    assert!(state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Retained {
            message: "retained test owner".into(),
            can_recover: true,
            can_release: false
        }
    ));
    let mut harness = harness(state);
    let recover = harness.state().0.localizer.text("editing-recover");
    harness
        .get_by_role_and_label(Role::Button, &recover)
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FieldDropSave { identity: actual, command: FieldDropSaveCommand::Recover }] if *actual == identity)
    );
    assert!(
        harness
            .state()
            .0
            .matches_field_drop_save(&request, identity)
    );
    harness.state_mut().1.clear();
    harness.key_press(Key::Escape);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    let (mut state, request, identity) = saving();
    state.set_field_drop_save_outcome(
        identity,
        EditSaveOutcome::Saved("illustrative known AFTER".into()),
    );
    assert!(state.begin_field_drop_reopen(identity));
    let mut harness = self::harness(state);
    harness.key_press(Key::Escape);
    harness.run_steps(1);
    assert!(
        matches!(harness.state().1.as_slice(), [Action::FieldDropSave { identity: actual, command: FieldDropSaveCommand::Cancel }] if *actual == identity)
    );
    assert!(
        harness
            .state()
            .0
            .matches_field_drop_save(&request, identity)
    );
}

#[test]
fn cancel_moves_exact_close_owner_even_when_other_retained_capacity_fills_budget() {
    let (mut state, request) = pending();
    assert!(state.request_current_field_drop_close());
    let retained = state
        .field_drop
        .close
        .as_ref()
        .unwrap()
        .target
        .column
        .as_ptr();
    state
        .data
        .as_mut()
        .unwrap()
        .table
        .reserve_exact(crate::workspace::objects::MAX_RETAINED_BYTES);
    let result = state
        .choose_field_drop_close(crate::FieldDropCloseDecision::Cancel)
        .unwrap();
    assert_eq!(result.target.column.as_ptr(), retained);
    assert_eq!(result.target, request.target);
    assert!(result.request.is_none());
    assert!(state.field_drop_dirty());
    assert!(state.field_drop_save_status().is_none());
    assert!(!state.field_drop_close_open());
}

#[test]
fn compact_and_normal_all_locale_controls_stay_inside_the_viewport() {
    for locale in Locale::ALL {
        for size in [egui::vec2(1280.0, 820.0), egui::vec2(860.0, 640.0)] {
            for mode in 0..4 {
                let mut state = match mode {
                    0 => observed().0,
                    1 => pending().0,
                    2 => {
                        let (mut state, request, identity) = saving();
                        state.set_field_drop_save_outcome(
                            identity,
                            EditSaveOutcome::Saved("illustrative outcome".into()),
                        );
                        state.begin_field_drop_reopen(identity);
                        state.fail_field_drop_reopen(
                            identity,
                            "illustrative full-image read failure".into(),
                        );
                        state.return_field_drop_to_draft(&request, identity, None);
                        state
                    }
                    _ => {
                        let (mut state, _) = pending();
                        state.request_current_field_drop_close();
                        state
                    }
                };
                state.localizer = Localizer::new(locale).unwrap();
                let mut harness = Harness::builder().with_size(size).build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
                crate::configure(&harness.ctx);
                harness.run_steps(3);
                let key = match mode {
                    0 => "field-drop-no",
                    1 => "backstage-close",
                    2 => "field-drop-close-editor",
                    _ => "field-drop-close-cancel",
                };
                let label = harness.state().0.localizer.text(key);
                let button = harness.get_by_role_and_label(Role::Button, &label);
                assert!(
                    harness.ctx.viewport_rect().contains_rect(button.rect()),
                    "{locale:?} mode{mode} {key}"
                );
                assert!(harness.state().1.is_empty());
            }
        }
    }
}

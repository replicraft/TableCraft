use super::*;
use crate::preview::{Preview, Scene};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

mod memory;

// The existing harness advances at four frames per second; this exceeds egui's
// one-second stable undo interval without wall-clock sleeps or history mutation.
const UNDO_STABLE_STEPS: usize = 8;

fn harness() -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::SavedError);
    let state = std::mem::take(preview.state_mut());
    Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .with_max_steps(32)
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        )
}

#[test]
fn parameter_undo_restores_longer_unicode_text_and_redo_keeps_committed_results_untouched() {
    let mut harness = harness();
    harness.run();
    let original = harness.state().0.saved_query.inputs.clone();
    let target = harness.state().0.saved_query.target.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let name = original[0].name.clone();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    let longer = "  Résumé 日本語 😀  ".repeat(64);
    let shorter = "é🦀";
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(longer.clone()));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        longer
    );
    harness.run_steps(UNDO_STABLE_STEPS);

    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(shorter.to_owned()));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        shorter
    );
    harness.run_steps(UNDO_STABLE_STEPS);

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        longer
    );
    assert!(
        !harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .input_rejected
    );
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        shorter
    );
    assert!(
        !harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .input_rejected
    );

    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(harness.state().0.saved_query.target, target);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn oversized_parameter_events_preserve_the_literal_and_require_a_real_correction() {
    let mut harness = harness();
    harness.run();
    let original = harness.state().0.saved_query.inputs.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let name = original[0].name.clone();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    harness.event(egui::Event::Text("keep this literal".into()));
    harness.run();
    let valid = harness
        .state()
        .0
        .saved_query
        .prompt
        .as_ref()
        .unwrap()
        .body
        .draft[0]
        .text
        .clone();

    let oversized = "x".repeat(1_048_577);
    harness.event(egui::Event::Text(oversized.clone()));
    harness.event(egui::Event::Paste(oversized.clone()));
    harness.event(egui::Event::Ime(egui::ImeEvent::Commit(oversized)));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        valid
    );
    assert!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .input_rejected
    );
    let label = harness.state().0.localizer.text("saved-query-input-limit");
    assert!(harness.query_by_label(&label).is_some());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().0.saved_query_prompt_open());
    assert!(harness.state().1.is_empty());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );

    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text("corrected".into()));
    harness.run();
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        "corrected"
    );
    assert!(
        !harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .input_rejected
    );
    assert!(harness.query_by_label(&label).is_none());
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.state().1.is_empty());
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
}

#[test]
fn pointer_focus_and_oversized_paste_in_one_frame_cannot_bypass_parameter_guard() {
    let mut harness = harness();
    harness.run();
    let original = harness.state().0.saved_query.inputs.clone();
    let name = original[0].name.clone();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    harness.event(egui::Event::Text("retained literal".into()));
    harness.run();
    let previous = harness
        .state()
        .0
        .saved_query
        .prompt
        .as_ref()
        .unwrap()
        .body
        .draft[0]
        .text
        .clone();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .focus();
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .is_focused()
    );

    let position = harness
        .get_by_role_and_label(Role::TextInput, &name)
        .rect()
        .center();
    harness.event(egui::Event::PointerMoved(position));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.event(egui::Event::Paste("z".repeat(1_048_577)));
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, &name)
            .is_focused()
    );
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .draft[0]
            .text,
        previous
    );
    assert!(
        harness
            .state()
            .0
            .saved_query
            .prompt
            .as_ref()
            .unwrap()
            .body
            .input_rejected
    );
    let label = harness.state().0.localizer.text("saved-query-input-limit");
    assert!(harness.query_by_label(&label).is_some());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert!(harness.state().1.is_empty());
}

#[test]
fn saved_parameter_editor_counts_unicode_characters_independently_of_utf8_bytes() {
    let mut harness = harness();
    harness.run();
    let original = harness.state().0.saved_query.inputs.clone();
    let name = original[0].name.clone();
    let limit = 65_535;
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, &name)
        .focus();
    harness.run();
    harness.event(egui::Event::Paste("雪".repeat(limit)));
    harness.run();
    let staged = &harness
        .state()
        .0
        .saved_query
        .prompt
        .as_ref()
        .unwrap()
        .body
        .draft[0]
        .text;
    assert_eq!(staged, &"雪".repeat(limit));
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );

    harness.event(egui::Event::Paste("b".into()));
    harness.run();
    let staged = &harness
        .state()
        .0
        .saved_query
        .prompt
        .as_ref()
        .unwrap()
        .body
        .draft[0]
        .text;
    assert_eq!(staged, &"雪".repeat(limit));
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert!(harness.state().1.is_empty());
}

#[test]
fn cancelling_staged_input_retains_committed_inputs_results_and_source_identity() {
    let mut harness = harness();
    harness.run();
    let original = harness.state().0.saved_query.inputs.clone();
    let target = harness.state().0.saved_query.target.clone();
    let rows = harness
        .state()
        .0
        .saved_query
        .data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = harness.state().0.sql.clone();
    let prompt = harness.state().0.saved_query.parameters.as_ref().unwrap()[0]
        .name
        .clone();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click();
    harness.run();
    assert!(harness.state().0.saved_query_prompt_open());
    harness
        .get_by_role_and_label(Role::TextInput, &prompt)
        .focus();
    harness.run();
    harness.event(egui::Event::Text("staged, never committed".into()));
    harness.run();
    assert_eq!(harness.state().0.saved_query.inputs, original);

    // Staging retains the textbox character policy without treating UTF-8 bytes
    // or parameter names as a shared data-service limit.
    harness
        .state_mut()
        .0
        .saved_query
        .prompt
        .as_mut()
        .unwrap()
        .body
        .draft[0]
        .text = "x".repeat(65_536);
    harness.run();
    let limit_label = harness.state().0.localizer.text("saved-query-input-limit");
    let reason = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == limit_label => Some((
                egui::Rect::from_min_size(text.pos, text.galley.size()),
                shape.clip_rect,
            )),
            _ => None,
        })
        .expect("the editor character-limit reason remains visible");
    assert!(reason.1.contains_rect(reason.0));
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .rect()
            .bottom()
            < 580.0
    );
    assert!(harness.state().1.is_empty());
    let backdrop = egui::pos2(1.0, 1.0);
    harness.event(egui::Event::PointerMoved(backdrop));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: backdrop,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    assert!(
        harness.state().0.saved_query_prompt_open(),
        "backdrop clicks do not dismiss acquisition"
    );
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(harness.state().0.saved_query.target, target);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert_eq!(harness.state().0.sql, sql);
    assert!(harness.state().1.is_empty());

    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert_eq!(harness.state().0.saved_query.inputs, original);
    assert_eq!(
        harness
            .state()
            .0
            .saved_query
            .data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        rows
    );
    assert!(
        harness.state().1.is_empty(),
        "Enter on Cancel cannot execute the query"
    );

    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click();
    harness.run();
    harness.state_mut().0.bump_focus_publication();
    harness.run();
    assert!(!harness.state().0.saved_query_prompt_open());
    assert!(
        harness.state().1.is_empty(),
        "a replaced publication cannot submit old staging"
    );
}

#[test]
fn final_confirmation_commits_once_and_retains_changed_input_result_invalidation() {
    let mut harness = harness();
    harness.run();
    let prompt = harness.state().0.saved_query.parameters.as_ref().unwrap()[0]
        .name
        .clone();
    let target = harness.state().0.saved_query.target.clone().unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, &prompt)
        .focus();
    harness.run();
    harness.event(egui::Event::Text("Portland".into()));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::RunSavedQuery { target: actual, parameters }]
        if *actual == target && parameters.len() == 1 && parameters[0].text == "Portland")
    );
    assert!(!harness.state().0.saved_query_prompt_open());
    assert!(harness.state().0.saved_query.data.is_none());
    harness.run_steps(3);
    assert_eq!(harness.state().1.len(), 1);
}

#[test]
fn painted_confirmation_group_aligns_with_the_value_field_without_reversing_button_order() {
    for (locale, theme) in [
        (crate::Locale::EnUs, crate::Theme::Light),
        (crate::Locale::De, crate::Theme::Dark),
    ] {
        let mut harness = harness();
        harness.state_mut().0.localizer = crate::Localizer::new(locale).unwrap();
        harness.state_mut().0.theme = theme;
        harness.run();
        let localizer = harness.state().0.localizer.clone();
        harness
            .get_by_role_and_label(Role::Button, &localizer.text("saved-query-run"))
            .click();
        harness.run();
        let prompt = &harness.state().0.saved_query.parameters.as_ref().unwrap()[0].name;
        let input = harness
            .get_by_role_and_label(Role::TextInput, prompt)
            .rect();
        let painted_button = |label: &str| {
            harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                        assert!(shape.clip_rect.contains_rect(bounds));
                        Some(bounds)
                    }
                    _ => None,
                })
                .expect("the complete confirmation label is actually painted")
        };
        let ok = painted_button(&localizer.text("saved-query-prompt-ok"));
        let cancel = painted_button(&localizer.text("saved-query-prompt-cancel"));
        assert!(ok.right() < cancel.left(), "OK precedes Cancel visually");
        assert!(
            (input.right() - cancel.right()).abs() <= 12.0,
            "footer text ends at the value field's right inset"
        );
        harness
            .get_by_role_and_label(Role::TextInput, prompt)
            .focus();
        harness.run();
        harness.key_press(Key::Tab);
        harness.run();
        if harness.state().0.saved_query.parameters.as_ref().unwrap()[0].nullable {
            harness.key_press(Key::Tab);
            harness.run();
        }
        assert!(
            harness
                .get_by_role_and_label(Role::Button, &localizer.text("saved-query-prompt-ok"))
                .is_focused(),
            "Tab reaches OK before Cancel"
        );
        harness.key_press(Key::Tab);
        harness.run();
        assert!(
            harness
                .get_by_role_and_label(Role::Button, &localizer.text("saved-query-prompt-cancel"))
                .is_focused()
        );
        assert!(harness.state().1.is_empty());
    }
}

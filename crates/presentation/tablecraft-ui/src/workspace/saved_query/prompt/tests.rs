use super::*;
use crate::preview::{Preview, Scene};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

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

    // Staging still obeys the aggregate byte allowance, including the prompt
    // name and kind in addition to the literal value being edited.
    harness
        .state_mut()
        .0
        .saved_query
        .prompt
        .as_mut()
        .unwrap()
        .draft[0]
        .text = "x".repeat(super::super::MAX_INPUT_BYTES);
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
        .expect("the existing aggregate-limit reason remains visible");
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

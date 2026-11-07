use super::*;
use crate::{Action, Locale, Localizer, State, Theme};
use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

#[test]
fn prompt_defaults_to_cancel_and_never_dispatches_background_shortcuts() {
    let mut state = State {
        file_picker_available: true,
        ..State::default()
    };
    state.show_recovery_prompt(7, "Example.accdb".into());
    state.fail_recovery_start(7, "Recovery could not be started".into());
    state.fail_recovery_start(8, "Stale diagnostic".into());
    let mut harness = harness(state);
    assert!(
        harness
            .get_all_by_value("Recovery could not be started")
            .any(|node| node.accesskit_node().role() == Role::Label)
    );
    assert!(harness.query_by_value("Stale diagnostic").is_none());
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .is_focused()
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::O);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CloseRecovery { attempt_id: 7 }]
    ));
    assert!(harness.state().0.database.is_none());
    assert!(harness.state().0.data.is_none());
}

#[test]
fn stale_ids_cannot_replace_cancel_or_acknowledge_a_retained_operation() {
    let mut state = State::default();
    state.show_recovery_prompt(7, "Captured.accdb".into());
    state.show_recovery_prompt(8, "Another.accdb".into());
    assert_eq!(state.recovery_attempt_id(), Some(7));
    assert!(!state.can_begin_recovery(8));
    assert!(!state.begin_recovery(8, 10));
    assert!(!state.begin_recovery(7, 0));
    assert!(state.begin_recovery(7, 10));
    assert!(state.recovery_pending());
    assert!(!state.close_recovery(7));
    state.set_recovery_outcome(9, RecoveryOutcome::Finished("Stale".into()));
    assert!(state.recovery_pending());
    state.set_recovery_outcome(
        10,
        RecoveryOutcome::Retained {
            message: "Unknown outcome".into(),
            can_recover: true,
            can_release: false,
        },
    );
    assert!(!state.recovery_pending());
    assert!(state.can_resolve_recovery(10, true));
    assert!(!state.can_resolve_recovery(10, false));
    assert!(!state.can_resolve_recovery(9, true));
    assert!(!state.can_acknowledge_recovery(10));
    assert!(!state.acknowledge_recovery(10));
    assert!(!state.begin_recovery_resolution(9, 11));
    assert!(state.begin_recovery_resolution(10, 11));
    state.set_recovery_outcome(10, RecoveryOutcome::Finished("Stale".into()));
    assert!(state.recovery_pending());
    state.set_recovery_outcome(
        11,
        RecoveryOutcome::Finished("Original image verified".into()),
    );
    assert!(state.can_acknowledge_recovery(11));
    assert!(!state.acknowledge_recovery(10));
    assert!(state.acknowledge_recovery(11));
    assert!(!state.recovery_prompt_open());
}

#[test]
fn running_escape_requests_cancel_once_and_retained_escape_never_releases_or_dismisses() {
    let mut state = State::default();
    state.show_recovery_prompt(7, "Example.accdb".into());
    assert!(state.begin_recovery(7, 10));
    let mut harness = harness(state);
    for _ in 0..2 {
        harness.key_press(Key::Escape);
        harness.run_steps(2);
    }
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CancelRecovery { operation_id: 10 }]
    ));
    harness.state_mut().1.clear();
    harness.state_mut().0.set_recovery_outcome(
        10,
        RecoveryOutcome::Retained {
            message: "Resources retained".into(),
            can_recover: false,
            can_release: true,
        },
    );
    harness
        .state_mut()
        .0
        .fail_recovery_resolution(10, "Release could not be started".into());
    harness
        .state_mut()
        .0
        .fail_recovery_resolution(9, "Stale diagnostic".into());
    harness.run_steps(2);
    assert!(
        harness
            .get_all_by_value("Release could not be started")
            .any(|node| node.accesskit_node().role() == Role::Label)
    );
    assert!(harness.query_by_value("Stale diagnostic").is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Recover")
            .is_none()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Release resources")
        .click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::ResolveRecovery {
            operation_id: 10,
            recover: false
        }]
    ));
    assert!(harness.state().0.recovery_prompt_open());
}

#[test]
fn all_languages_and_themes_render_the_source_free_prompt_and_neutral_result() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let mut state = State {
                localizer: Localizer::new(locale).unwrap(),
                theme,
                ..State::default()
            };
            state.show_recovery_prompt(7, "Example.accdb".into());
            let mut harness = harness(state);
            let recover = harness.state().0.localizer.text("recovery-recover");
            harness
                .get_by_role_and_label(Role::Button, &recover)
                .click_accesskit();
            harness.run_steps(2);
            assert!(matches!(
                harness.state().1.as_slice(),
                [Action::BeginRecovery { attempt_id: 7 }]
            ));
            assert!(harness.state_mut().0.begin_recovery(7, 10));
            harness.state_mut().0.set_recovery_outcome(
                10,
                RecoveryOutcome::Finished(
                    "Original image verified; no historical commit asserted.".into(),
                ),
            );
            harness.run_steps(2);
            let heading = harness.state().0.localizer.text("recovery-result");
            assert!(
                harness
                    .get_all_by_value(&heading)
                    .any(|node| node.accesskit_node().role() == Role::Label)
            );
            assert!(
                harness
                    .get_all_by_value("Original image verified; no historical commit asserted.")
                    .any(|node| node.accesskit_node().role() == Role::Label)
            );
            assert!(harness.state().0.database.is_none());
            assert!(harness.state().0.data.is_none());
        }
    }
}

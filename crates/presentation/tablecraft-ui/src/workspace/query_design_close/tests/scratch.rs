//! Actual SQL authoring retains an unsaved close owner independently of Design.
use super::*;

fn scratch() -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::QuerySaveName);
    let mut state = std::mem::take(preview.state_mut());
    assert!(state.cancel_query_creation_name());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 780.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn text(harness: &mut Harness<'_, (State, Vec<Action>)>, text: &str) {
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    if text.is_empty() {
        harness.key_press(Key::Backspace);
    } else {
        harness.event(Event::Paste(text.into()));
    }
    harness.run_steps(2);
}

#[test]
fn exact_empty_baseline_is_clean_even_after_real_text_edits_and_new_revisions() {
    let mut harness = scratch();
    text(&mut harness, "");
    assert!(!harness.state().0.has_unsaved_scratch_query());
    let empty_revision = harness.state().0.query_sql_revision().unwrap();
    text(&mut harness, "SELECT 7 AS [Authored]");
    assert!(harness.state().0.has_unsaved_scratch_query());
    assert!(harness.state().0.query_sql_revision().unwrap() > empty_revision);
    text(&mut harness, "");
    assert!(!harness.state().0.has_unsaved_scratch_query());
    let tab = harness.state().0.active_object().unwrap();
    assert!(harness.state_mut().0.close_object(&tab));
    assert!(!harness.state().0.query_design_close_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn sql_close_cancel_discard_and_stale_save_preserve_the_exact_current_author() {
    let mut harness = scratch();
    text(&mut harness, "SELECT 7 AS [Authored]");
    let tab = harness.state().0.active_object().unwrap();
    assert!(!harness.state_mut().0.close_object(&tab));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run_steps(2);
    assert_eq!(harness.state().0.active_object().as_ref(), Some(&tab));
    assert_eq!(harness.state().0.sql, "SELECT 7 AS [Authored]");

    let state = &mut harness.state_mut().0;
    assert!(state.request_query_design_close(&tab));
    let result = state
        .choose_query_design_close(QueryDesignCloseDecision::Save)
        .unwrap();
    assert_eq!(result.request.draft_revision, None);
    assert!(state.replace_query_sql("SELECT 8 AS [Newer]".into()));
    assert!(
        state
            .begin_query_design_close_save(&result.request)
            .is_none()
    );
    assert!(!state.query_creation_name_open());
    let mut events = Vec::new();
    state.finish_query_design_close_events(&mut events);
    assert!(
        matches!(events.as_slice(), [Action::QueryDesignClose { result }] if result.decision == QueryDesignCloseDecision::Cancel)
    );
    assert_eq!(state.sql, "SELECT 8 AS [Newer]");

    assert!(state.request_query_design_close(&tab));
    assert!(
        state
            .choose_query_design_close(QueryDesignCloseDecision::Discard)
            .is_some()
    );
    assert!(state.sql.is_empty());
    assert!(!state.object_tabs().iter().any(|open| open.id == tab.id));
}

#[test]
fn sql_close_save_uses_name_directly_and_name_cancel_revokes_its_continuation() {
    let mut harness = scratch();
    text(&mut harness, "SELECT 9 AS [Authored]");
    let state = &mut harness.state_mut().0;
    let tab = state.active_object().unwrap();
    assert!(state.request_query_design_close(&tab));
    let result = state
        .choose_query_design_close(QueryDesignCloseDecision::Save)
        .unwrap();
    assert!(matches!(
        state.begin_query_design_close_save(&result.request),
        Some(QueryDesignCloseSave::Naming)
    ));
    assert!(state.query_creation_name_open());
    assert!(state.cancel_query_creation_name());
    assert!(state.query_design_close.intent.is_none());
    let mut events = Vec::new();
    state.finish_query_design_close_events(&mut events);
    assert!(
        matches!(events.as_slice(), [Action::QueryDesignClose { result }] if result.decision == QueryDesignCloseDecision::Cancel)
    );
    assert_eq!(state.active_object().as_ref(), Some(&tab));
    assert_eq!(state.sql, "SELECT 9 AS [Authored]");
}

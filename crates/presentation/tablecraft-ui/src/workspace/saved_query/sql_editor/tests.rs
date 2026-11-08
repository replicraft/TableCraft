use crate::workspace::active_save::tests::{harness, inspected_state};
use crate::workspace::{Action, SavedQuerySqlEligibility};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn inert_sql_is_visible_but_host_opt_in_is_required_for_editing_and_save() {
    let (state, _) = inspected_state(false);
    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "Save")
            .all(|save| save.accesskit_node().is_disabled())
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.saved_query_sql_dirty());
}

#[test]
fn qat_save_emits_exact_owned_text_and_disables_run_for_the_dirty_definition() {
    let (state, target) = inspected_state(true);
    let mut harness = harness(state);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    let exact = "  SELECT 43 AS [Answer]; -- keep this formatting\n";
    harness.event(Event::Paste(exact.into()));
    harness.run_steps(1);
    assert_eq!(harness.state().0.saved_query_sql().unwrap().1, exact);
    assert!(!harness.state().0.saved_query_command_ready());
    assert!(!harness.state().0.can_run_saved_query(&target, &[]));
    assert!(harness.state_mut().0.request_saved_query_run().is_none());
    assert!(harness.state().1.is_empty());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Run saved query")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_all_by_role_and_label(Role::Button, "Save")
        .min_by(|left, right| left.rect().top().total_cmp(&right.rect().top()))
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    let [Action::ReviewSavedQuerySql { request }] = harness.state().1.as_slice() else {
        panic!("QAT must issue only the exact saved-query review");
    };
    assert_eq!(request.target, target);
    assert_eq!(
        request.original_sql.as_deref(),
        Some("SELECT 42 AS [Answer];")
    );
    assert_eq!(request.proposed_sql, exact);
    assert_eq!(
        request.draft_revision,
        harness.state().0.saved_query_sql().unwrap().2
    );
}

#[test]
fn shortcut_captures_an_editor_update_from_the_same_frame_after_widgets() {
    let (state, target) = inspected_state(true);
    assert!(!state.can_save_active_object());
    let mut harness = harness(state);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    let exact = "SELECT 44 AS [Answer];";
    harness.event(Event::Paste(exact.into()));
    harness.event(Event::Key {
        key: Key::S,
        physical_key: Some(Key::S),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    });
    harness.run_steps(1);
    let [Action::ReviewSavedQuerySql { request }] = harness.state().1.as_slice() else {
        panic!("shortcut must issue one review after the editor update");
    };
    assert_eq!(request.target, target);
    assert_eq!(request.proposed_sql, exact);
    assert_eq!(
        request.draft_revision,
        harness.state().0.saved_query_sql().unwrap().2
    );
}

#[test]
fn unqualified_sources_have_no_editor_and_no_save_dispatch() {
    let (mut state, target) = inspected_state(true);
    let request = state.begin_saved_query_sql_inspection(target, 3).unwrap();
    assert!(state.set_saved_query_sql_inspection(
        3,
        &request,
        None,
        SavedQuerySqlEligibility::Unavailable
    ));
    let mut harness = harness(state);
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .is_none()
    );
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "Save")
            .all(|save| save.accesskit_node().is_disabled())
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
}

#[test]
fn saved_sql_layout_uses_a_bounded_viewport_and_keeps_text_unchanged() {
    let (mut state, _) = inspected_state(true);
    let exact = "SELECT 43 AS [Answer];\n-- retained line\n".repeat(45);
    assert!(state.set_saved_query_sql_text(exact.clone()));
    let mut harness = harness(state);
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::MultilineTextInput, "SQL query");
    let clip = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == exact => Some(shape.clip_rect),
            _ => None,
        })
        .expect("the actual saved SQL is painted through its bounded editor");
    assert!(clip.height() <= super::MAX_SQL_EDITOR_HEIGHT + 2.0);
    assert_eq!(harness.state().0.saved_query_sql().unwrap().1, exact);
    assert!(harness.state().1.is_empty());
}

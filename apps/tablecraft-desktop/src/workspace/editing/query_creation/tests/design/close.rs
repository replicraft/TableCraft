//! Real close choices consume only the original Design owner's completed save.
use super::super::{accept_name, name, opened, run_saved, saved, scratch, sql};
use super::design;
use crate::workspace::{Desktop, test_support::finish_schema_save};
use egui::{Event, Id, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{Command, journal_path};
use tablecraft_testkit::fixtures::access::query_expression_edits;
use tablecraft_ui::ObjectKind;

fn edit(harness: &mut Harness<'_, Desktop>, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste(text.into()));
    harness.run_steps(2);
    assert!(harness.state().state.has_modified_query_design());
}

fn dirty_scratch(harness: &mut Harness<'_, Desktop>) {
    scratch(harness);
    sql(harness, "SELECT 98 AS Answer;");
    design(harness);
    edit(harness, "Answer: 99");
}

fn close(harness: &mut Harness<'_, Desktop>) {
    let tab = harness.state().state.active_object().unwrap();
    harness.state_mut().close_object(tab);
    harness.run_steps(2);
    assert!(harness.state().state.query_design_close_open());
}

fn choose(harness: &mut Harness<'_, Desktop>, key: &str) {
    let label = harness.state().state.localizer.text(key);
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("query_design_close")))
        .expect("actual Design close modal");
    let choices = harness
        .get_all_by_role_and_label(Role::Button, &label)
        .filter(|button| modal.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(choices.len(), 1, "exact choice in the real close modal");
    assert!(!choices[0].accesskit_node().is_disabled());
    choices[0].click_accesskit();
    drop(choices);
    harness.run_steps(1);
}

fn fill_name_after_close_save(harness: &mut Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !harness.state().state.query_creation_name_open() {
        assert!(
            Instant::now() < deadline,
            "real close Save Build must reach Name"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness
        .get_by_role_and_label(Role::TextInput, "Query Name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste("Created".into()));
    harness.run_steps(1);
}

#[test]
fn deferred_open_name_cancel_cannot_arm_a_later_ordinary_save_to_close_or_replace_the_tab() {
    let (directory, path, case, clock, mut harness) = opened();
    dirty_scratch(&mut harness);
    let id = harness.state().state.active_object().unwrap().id;
    let other = directory.path().join("other.accdb");
    std::fs::write(&other, &case.before).unwrap();
    // NO_CLONE: The actual deferred Open owns its path while the observer keeps
    // the other file available to prove that cancellation never resumed that open.
    harness.state_mut().open_request(Command::Open {
        path: other.clone(),
        password: String::new(),
    });
    harness.run_steps(2);
    assert!(harness.state().state.query_design_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    choose(&mut harness, "saved-query-close-save");
    fill_name_after_close_save(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.query_creation_name_open());
    assert!(harness.state().state.has_modified_query_design());
    assert_eq!(harness.state().state.active_object().unwrap().id, id);
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);

    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_eq!(active.id, id);
    let ObjectKind::SavedQuery { name, .. } = &active.target.kind else {
        panic!("ordinary Save must retain the actual created saved tab");
    };
    assert_eq!(name, "Created");
    assert!(harness.state().state.query_design_draft().is_some());
    assert!(!harness.state().state.has_modified_query_design());
    assert!(!harness.state().editing.has_save());
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
    assert_eq!(std::fs::read(other).unwrap(), case.before);
}

#[test]
fn close_save_name_ok_waits_for_created_full_witness_then_closes_only_that_source() {
    let (_directory, path, case, clock, mut harness) = opened();
    dirty_scratch(&mut harness);
    let id = harness.state().state.active_object().unwrap().id;
    close(&mut harness);
    choose(&mut harness, "saved-query-close-save");
    fill_name_after_close_save(&mut harness);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == id)
    );
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == id)
    );
    assert!(!harness.state().state.has_modified_query_design());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert!(!journal_path(&path).exists());
    saved(&mut harness, "Created");
    run_saved(&mut harness, "99");
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
}

#[test]
fn cancel_keeps_the_actual_dirty_design_and_discard_closes_it_without_backend_work() {
    let (_directory, path, case, clock, mut harness) = opened();
    dirty_scratch(&mut harness);
    let id = harness.state().state.active_object().unwrap().id;
    close(&mut harness);
    choose(&mut harness, "saved-query-close-cancel");
    harness.run_steps(2);
    assert_eq!(harness.state().state.active_object().unwrap().id, id);
    assert!(harness.state().state.has_modified_query_design());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Answer: 99")
    );
    assert!(!harness.state().editing.has_save());
    close(&mut harness);
    choose(&mut harness, "saved-query-close-discard");
    harness.run_steps(2);
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == id)
    );
    assert!(!harness.state().state.has_modified_query_design());
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    assert!(!journal_path(&path).exists());
}

#[test]
fn saved_design_close_save_uses_existing_query_intent_and_preserves_native_identity() {
    let (_directory, path, case, clock, mut harness) = opened();
    let expected = query_expression_edits::replace(&case.before, "43");
    saved(&mut harness, "Saved");
    let id = harness.state().state.active_object().unwrap().id;
    design(&mut harness);
    edit(&mut harness, "Answer: 43");
    let target = harness.state().state.selected_saved_target().unwrap();
    assert!(
        !harness.state().state.can_run_saved_query(target, &[]),
        "stored Run cannot ignore dirty Design before Build"
    );
    close(&mut harness);
    choose(&mut harness, "saved-query-close-save");
    finish_schema_save(&mut harness);
    assert!(!harness.state().state.query_creation_name_open());
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == id)
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(
        clock.calls(),
        0,
        "existing-query save cannot take creation metadata"
    );
    assert!(
        !harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .queries
            .iter()
            .any(|query| query.name == "Created")
    );
    saved(&mut harness, "Saved");
    run_saved(&mut harness, "43");
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

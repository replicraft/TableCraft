//! Scratch SQL has no native saved baseline; close Save uses genuine creation.
use super::*;
use crate::workspace::test_support::finish_reads;

#[test]
fn scratch_sql_deferred_open_name_cancel_cannot_rearm_a_later_ordinary_save() {
    let (directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 98 AS [Answer]");
    design(&mut harness);
    edit(&mut harness, "Answer: 101");
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "101"
    );
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(2);
    sql(&mut harness, "SELECT 99 AS [Answer]");
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "101",
        "retained historical Design results do not own newer authored SQL99"
    );
    assert!(!harness.state().state.has_modified_query_design());
    let id = harness.state().state.active_object().unwrap().id;
    let other = directory.path().join("other.accdb");
    std::fs::write(&other, &case.before).unwrap();
    // NO_CLONE: The actual deferred command owns its path while the observer
    // retains the independent file to prove that Name Cancel never opens it.
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
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(harness.state().state.active_object().unwrap().id, id);
    assert_eq!(harness.state().state.sql, "SELECT 99 AS [Answer]");
    assert!(harness.state().state.has_unsaved_scratch_query());
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);

    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_eq!(active.id, id);
    assert!(
        matches!(&active.target.kind, ObjectKind::SavedQuery { name, .. } if name == "Created")
    );
    assert!(harness.state().state.query_design_draft().is_none());
    assert!(!harness.state().state.has_unsaved_scratch_query());
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
    assert_eq!(std::fs::read(other).unwrap(), case.before);
}

#[test]
fn scratch_sql_close_save_waits_for_real_created_ack_and_full_witness() {
    let (_directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 98 AS [Answer]");
    design(&mut harness);
    edit(&mut harness, "Answer: 101");
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    sql(&mut harness, "SELECT 99 AS [Answer]");
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        !harness.state().state.has_modified_query_design(),
        "SQL results do not revive the older 101 Design close owner"
    );
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
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert!(!journal_path(&path).exists());
    saved(&mut harness, "Created");
    run_saved(&mut harness, "99");
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
}

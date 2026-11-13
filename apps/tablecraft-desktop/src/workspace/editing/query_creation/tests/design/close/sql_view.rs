//! Current authored SQL supersedes a dormant Design model for Run and close Save.
use super::super::super::{opened, run_saved, saved, sql};
use super::super::design;
use super::edit;
use crate::workspace::test_support::{finish_reads, finish_schema_save};
use egui::{Id, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::query_expression_edits;

#[test]
fn checked_design_then_authored_sql_uses_current_saved_sql_run_and_tcqe_close() {
    let (_directory, path, case, clock, mut harness) = opened();
    let expected = query_expression_edits::replace(&case.before, "44");
    saved(&mut harness, "Saved");
    let tab = harness.state().state.active_object().unwrap();
    // NO_CLONE: This exact stored baseline is later submitted through the real
    // editor to prove that the dormant changed Design cannot block a clean SQL Run.
    let original = harness
        .state()
        .state
        .saved_query_sql()
        .unwrap()
        .0
        .to_string();
    design(&mut harness);
    edit(&mut harness, "Answer: 43");
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(
        harness
            .state()
            .state
            .saved_query_sql()
            .unwrap()
            .1
            .contains("43")
    );
    harness.get_by_role_and_label(Role::MultilineTextInput, "SQL query");

    sql(&mut harness, &original);
    assert!(!harness.state().state.saved_query_sql_dirty());
    let target = harness.state().state.selected_saved_target().unwrap();
    assert!(
        harness.state().state.can_run_saved_query(target, &[]),
        "a dormant Design cannot block current clean stored SQL"
    );
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Run saved query")
            .accesskit_node()
            .is_disabled()
    );
    run_saved(&mut harness, "42");
    assert_eq!(std::fs::read(&path).unwrap(), case.before);

    sql(&mut harness, "SELECT 44 AS Answer;");
    assert!(harness.state().state.saved_query_sql_dirty());
    let closing = harness.state().state.active_object().unwrap();
    harness.state_mut().close_object(closing);
    harness.run_steps(2);
    assert!(harness.state().state.saved_query_sql_close_open());
    assert!(!harness.state().state.query_design_close_open());
    assert_eq!(clock.calls(), 0);
    let modal = harness
        .ctx
        .memory(|memory| memory.area_rect(Id::new("saved_query_sql_close")))
        .expect("actual current SQL close confirmation");
    let choices = harness
        .get_all_by_role_and_label(Role::Button, "Save")
        .filter(|button| modal.contains_rect(button.rect()))
        .collect::<Vec<_>>();
    assert_eq!(choices.len(), 1);
    assert!(!choices[0].accesskit_node().is_disabled());
    choices[0].click_accesskit();
    drop(choices);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert!(!harness.state().state.query_creation_name_open());
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|open| open.id == tab.id)
    );
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert_eq!(
        clock.calls(),
        0,
        "existing SQL save must not enter new-query metadata review"
    );
    saved(&mut harness, "Saved");
    run_saved(&mut harness, "44");
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(clock.calls(), 0);
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
}

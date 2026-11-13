//! A real unsupported declaration keeps its authored retry data and prior result.
use super::{accept_name, name, opened, scratch, sql};
use crate::workspace::test_support::finish_reads;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;

#[test]
fn unsupported_constant_shape_retains_name_sql_source_and_previous_result_without_save() {
    let (_directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 7 AS Previous;");
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(&mut harness, "SELECT 1.5 AS Answer;");
    let tab = harness.state().state.active_object().unwrap();
    name(&mut harness, false);
    accept_name(&mut harness);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.has_save() {
        assert!(
            Instant::now() < deadline,
            "unsupported native declaration must finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    assert!(harness.state().state.query_creation_name_open());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Query Name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Created"),
    );
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert_eq!(harness.state().state.sql, "SELECT 1.5 AS Answer;");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert!(
        harness
            .state()
            .state
            .error
            .as_ref()
            .is_some_and(|message| message.contains("constant SELECT"))
    );
    assert_eq!(
        clock.calls(),
        1,
        "metadata review was attempted once; no save or retry rereads it"
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.query_creation_name_open());
    assert_eq!(harness.state().state.sql, "SELECT 1.5 AS Answer;");
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), case.before);
}

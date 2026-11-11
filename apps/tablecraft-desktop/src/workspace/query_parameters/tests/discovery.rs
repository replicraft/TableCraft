use super::{confirm, enter, finish_reads, opened, run};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::sync::Arc;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn normal_sql_run_prompts_for_unknown_literal_name_cancels_then_runs_exact_value() {
    let (_directory, path, original, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = "SELECT [Enter label:] AS Prompt,Id FROM Items WHERE Id=1";
    run(&mut harness, sql);
    assert!(harness.state().state.parameter_prompt_open());
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
    let inspected = harness.state().query_parameters.as_ref().unwrap();
    let logical = inspected.inspection_id;
    let discovered = Arc::clone(inspected.discovered.as_ref().unwrap());
    assert!(discovered.has_implicit_mode());
    assert_ne!(
        logical,
        harness.state().generation,
        "metadata worker request retains a distinct generation"
    );
    let generation = harness.state().generation;
    // Duplicate metadata replies cannot dismiss or replace the accepted prompt.
    harness.state_mut().publish_discovered_query_parameters(
        generation,
        Arc::clone(&discovered),
        Vec::new(),
    );
    assert!(harness.state().state.parameter_prompt_open());
    assert!(
        harness
            .state()
            .state
            .query_parameter_inspection_current(logical, sql)
    );
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.parameter_prompt_open());
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
    assert_eq!(harness.state().state.sql, sql);
    assert_image(&std::fs::read(&path).unwrap(), &original);
    harness.state_mut().publish_discovered_query_parameters(
        generation,
        Arc::clone(&discovered),
        Vec::new(),
    );
    assert!(
        !harness.state().state.parameter_prompt_open(),
        "cancelled metadata reply must not reopen a prompt"
    );
    run(&mut harness, sql);
    let current = harness.state().query_parameters.as_ref().unwrap();
    let current_id = current.inspection_id;
    let current_owner = Arc::clone(current.discovered.as_ref().unwrap());
    assert_ne!(current_id, logical);
    harness
        .state_mut()
        .publish_discovered_query_parameters(generation, discovered, Vec::new());
    let retained = harness.state().query_parameters.as_ref().unwrap();
    assert_eq!(retained.inspection_id, current_id);
    assert!(Arc::ptr_eq(
        retained.discovered.as_ref().unwrap(),
        &current_owner
    ));
    assert!(
        harness
            .state()
            .state
            .query_parameter_inspection_current(current_id, sql)
    );
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "OK")
            .accesskit_node()
            .is_disabled(),
        "VALUE input requires an explicitly selected carrier"
    );
    harness
        .get_by_role_and_label(Role::ComboBox, "Enter label:")
        .click_accesskit();
    harness.run_steps(1);
    harness.get_by_label("Text").click_accesskit();
    harness.run_steps(1);
    enter(&mut harness, "Enter label:", "日本語 🦀");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert!(!harness.state().state.parameter_prompt_open());
    assert!(
        harness.state().state.error.is_none(),
        "query failed: {:?}",
        harness.state().state.error
    );
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0].text, "日本語 🦀");
    assert_eq!(result.rows[0][1].text, "1");
    assert!(result.source_revision.is_none());
    assert_image(&std::fs::read(&path).unwrap(), &original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}

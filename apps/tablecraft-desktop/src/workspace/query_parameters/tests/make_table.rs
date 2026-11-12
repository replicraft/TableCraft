//! Normal SQL Run confirms typed inputs before the separate native make-table decision.
use super::{confirm, enter, opened_with_source, run};
use crate::workspace::test_support::{finish_reads, finish_save};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::fs;
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};
use tablecraft_ui::FluentArgs;

const SQL: &str = "PARAMETERS [Min] LONG; SELECT i.CopiedScore INTO Archive FROM Items AS i WHERE i.CopiedScore>=[Min] ORDER BY i.CopiedScore";

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn make_table_parameters_use_normal_prompts_and_no_yes_atomic_save_and_guarded_reopen() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[None, Some(1), Some(5), Some(9)]);
    let after = populated::numbers(&before, "Archive", &[Some(5), Some(9)]);
    let (_directory, path, original, mut harness) = opened_with_source(before);
    run(&mut harness, "SELECT 99 AS Previous");
    let sql_tab = harness.state().state.active_object().unwrap();
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();

    run(&mut harness, SQL);
    harness.get_by_role_and_label(Role::TextInput, "Min");
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_action_review.is_none());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    same(&fs::read(&path).unwrap(), &original);
    assert!(!journal_path(&path).exists());
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

    // Parameter confirmation is not native write confirmation. No preserves both
    // the original image and the previous published query relation.
    run(&mut harness, SQL);
    enter(&mut harness, "Min", "5");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert!(harness.state().state.query_action_dialog_open());
    same(&fs::read(&path).unwrap(), &original);
    assert!(!harness.state().editing.has_save());
    let mut arguments = FluentArgs::new();
    arguments.set("count", "2");
    let count = harness
        .state()
        .state
        .localizer
        .format("query-action-schema-make-table-records", &arguments);
    harness.get_by_role_and_label(Role::Label, &count);
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    same(&fs::read(&path).unwrap(), &original);
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
    assert!(!journal_path(&path).exists());

    // A fresh prompt supplies the original typed value to a new consuming review.
    run(&mut harness, SQL);
    enter(&mut harness, "Min", "5");
    confirm(&mut harness);
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        matches!(finish_save(&mut harness).outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 1 })
        && receipt.matched_rows.is_none() && receipt.changed_rows == Some(2))
    );
    same(&fs::read(&path).unwrap(), &after);
    assert!(harness.state().editing.has_save());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        sql_tab.id
    );
    assert_eq!(harness.state().state.sql, SQL);
    assert!(
        harness.state().state.query_data.is_none(),
        "ACK does not rerun SELECT INTO"
    );
    assert!(!journal_path(&path).exists());
    same(&fs::read(&path).unwrap(), &after);

    run(
        &mut harness,
        "SELECT CopiedScore FROM Archive ORDER BY CopiedScore",
    );
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "5");
    assert_eq!(rows[1][0].text, "9");
}

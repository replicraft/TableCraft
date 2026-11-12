//! An actual created index remains usable through normal reviewed SQL UPDATE.
mod inserting;

use super::{
    assert_image, cancel_refused_action, finish_reads, finish_refused_action, finish_save,
    journal_path, opened, reviewed, run,
};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange};
use tablecraft_testkit::fixtures::access::secondary_index::normal::updating;
use tablecraft_ui::FluentArgs;

fn create(harness: &mut Harness<'_, Desktop>, sql: &str, expected: &[u8], path: &std::path::Path) {
    reviewed(harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::IndexCreated)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert_image(&std::fs::read(path).unwrap(), expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert!(!journal_path(path).exists());
}

#[test]
fn created_index_updates_no_then_yes_and_fresh_sql_reads_the_rebuilt_key_order() {
    for descending in [false, true] {
        for case in [
            updating::incremented(descending),
            updating::protected_incremented(descending),
        ] {
            let source = case.creation;
            let create_sql = format!(
                "CREATE INDEX [{}] ON [{}] ([{}] {})",
                source.index_name,
                source.table,
                source.column,
                if descending { "DESC" } else { "ASC" }
            );
            let sql = format!(
                "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
                source.table, source.column, source.column
            );
            let select = format!(
                "SELECT [{}] FROM [{}] ORDER BY [{}]",
                source.column, source.table, source.column
            );
            let (_directory, path, _original, mut harness) = opened(source.before);
            create(&mut harness, &create_sql, &source.expected, &path);
            run(&mut harness, &select);
            let previous = harness
                .state()
                .state
                .query_data
                .as_ref()
                .unwrap()
                .rows
                .as_ptr();
            let discarded = reviewed(&mut harness, &sql);
            for key in ["query-action-matched", "query-action-update-changed"] {
                let mut args = FluentArgs::new();
                args.set("count", case.cells.len().to_string());
                let label = harness.state().state.localizer.format(key, &args);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Label, &label)
                        .is_some()
                );
            }
            assert_image(&std::fs::read(&path).unwrap(), &source.expected);
            assert!(!journal_path(&path).exists());
            harness
                .get_by_role_and_label(Role::Button, "No")
                .click_accesskit();
            harness.run_steps(2);
            harness.state_mut().apply_query_action(discarded);
            assert!(!harness.state().editing.has_save());
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
            assert_image(&std::fs::read(&path).unwrap(), &source.expected);

            reviewed(&mut harness, &sql);
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .click_accesskit();
            harness.run_steps(1);
            let report = finish_save(&mut harness);
            assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.matched_rows == Some(case.cells.len())
                && receipt.changed_rows == Some(case.cells.len()) && receipt.schema_change.is_none()));
            assert_image(&std::fs::read(&path).unwrap(), &case.expected);
            harness
                .get_by_role_and_label(Role::Button, "Close")
                .click_accesskit();
            harness.run_steps(1);
            finish_reads(&mut harness);
            assert!(!harness.state().editing.has_save());
            assert!(!harness.state().state.native_source_stale());
            assert_eq!(harness.state().state.sql, sql);
            assert!(
                harness.state().state.query_data.is_none(),
                "save does not replay SQL"
            );
            run(&mut harness, &select);
            let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
            let mut expected = case.cells.iter().map(|cell| cell.value).collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(rows.len(), expected.len());
            for (row, value) in rows.iter().zip(expected) {
                assert_eq!(
                    row[0].text,
                    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
                );
            }
            assert_image(&std::fs::read(&path).unwrap(), &case.expected);
            assert!(!journal_path(&path).exists());
        }
    }
}

#[test]
fn late_assignment_failure_preserves_the_created_index_and_previous_published_rows() {
    let case = updating::incremented(false);
    let source = case.creation;
    let create_sql = format!(
        "CREATE INDEX [{}] ON [{}] ([{}])",
        source.index_name, source.table, source.column
    );
    let select = format!(
        "SELECT [{}] FROM [{}] ORDER BY [{}]",
        source.column, source.table, source.column
    );
    let sql = format!(
        "UPDATE [{}] SET [{}]=10 \\ (7-[{}])",
        source.table, source.column, source.column
    );
    let (_directory, path, _original, mut harness) = opened(source.before);
    create(&mut harness, &create_sql, &source.expected, &path);
    run(&mut harness, &select);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    // NO_CLONE: The editor owns its SQL; retain an independent expected string
    // to verify the refused action preserves the exact submitted statement.
    assert!(harness.state_mut().state.replace_query_sql(sql.clone()));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_refused_action(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.sql, sql);
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
    assert_image(&std::fs::read(&path).unwrap(), &source.expected);
    assert!(!journal_path(&path).exists());
    cancel_refused_action(&mut harness);
    run(&mut harness, &select);
    assert_image(&std::fs::read(&path).unwrap(), &source.expected);
}

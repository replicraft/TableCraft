//! Real SQL Run → native schema review → explicit decision → guarded publication.
use super::{finish_reads, finish_save, opened, reviewed, run};
use crate::workspace::{Desktop, PendingOperation};
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
#[cfg(unix)]
use std::time::{Duration, Instant};
use tablecraft_runtime::{SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_creation, canonical_native17_creation,
    canonical_retained_add_table::{field_changes, fields, typed_fields},
    canonical_table_drop::retained as dropped,
};
use tablecraft_ui::ObjectKind;

fn close_target(harness: &mut Harness<'_, Desktop>, target: &str) {
    let tabs = harness.state().state.object_tabs();
    for tab in tabs {
        if matches!(&tab.target.kind, ObjectKind::Table { name } if name == target) {
            harness.state_mut().close_object(tab);
        }
    }
    harness.run_steps(2);
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing byte"
    );
}

fn journey(before: Vec<u8>, expected: Vec<u8>, sql: &str, table: &str, change: SaveSchemaChange) {
    creation_journey(before, expected, sql, table, change, None);
}

fn creation_journey(
    before: Vec<u8>,
    expected: Vec<u8>,
    sql: &str,
    table: &str,
    change: SaveSchemaChange,
    inserted: Option<usize>,
) {
    let (_directory, path, original, mut harness) = opened(before);
    close_target(&mut harness, table);
    let sql_tab = harness.state().state.active_object().unwrap();
    let rows = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let first = reviewed(&mut harness, sql);
    assert_image(&std::fs::read(&path).unwrap(), &original);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    harness.state_mut().apply_query_action(first);
    assert!(harness.state().editing.query_action_review.is_none());
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
        rows
    );
    assert_image(&std::fs::read(&path).unwrap(), &original);

    reviewed(&mut harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(change) && receipt.changed_rows == inserted && receipt.matched_rows.is_none() && receipt.query_change.is_none()));
    assert!(harness.state().editing.has_save());
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.query_schema.is_none());
    assert!(!harness.state().state.native_source_stale());
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        sql_tab.id
    );
    assert!(
        harness.state().state.query_data.is_none(),
        "DDL does not silently rerun SQL"
    );
    assert!(!journal_path(&path).exists());
    assert_image(&std::fs::read(&path).unwrap(), &expected);
}

#[test]
fn create_table_run_uses_real_native_definition_and_no_design_draft() {
    let before = canonical_blank_creation::source();
    let after = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    journey(
        before,
        after,
        "CREATE TABLE Items (Id INTEGER, Score LONG)",
        "Items",
        SaveSchemaChange::FirstTableCreated { column_count: 2 },
    );
}

#[test]
fn alter_drop_column_run_preserves_real_non_tail_native_identity_and_siblings() {
    let source = fields::source(fields::Case::FirstNumber);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let after = field_changes::drop_field(&before, source.definition, source.catalog_root, "Score");
    journey(
        before,
        after,
        "ALTER TABLE [tAbLe1] DROP COLUMN [sCoRe]",
        "Table1",
        SaveSchemaChange::ColumnDropped,
    );
}

#[test]
fn drop_table_run_reopens_only_its_actual_table_dropped_witness() {
    let source = dropped::source(dropped::Case::FirstNumber);
    let after = dropped::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        &source.roots,
    );
    journey(
        source.bytes,
        after,
        "DROP TABLE Table1",
        "Table1",
        SaveSchemaChange::TableDropped,
    );
}

#[test]
fn stale_sql_and_open_target_cannot_start_schema_save_and_escape_preserves_bytes() {
    let source = fields::source(fields::Case::FirstNumber);
    let (_directory, path, original, mut harness) = opened(source.bytes);
    // Open the actual issued native target; SQL DROP has a current closed-target
    // UI boundary and must not silently discard even a clean table tab.
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness
        .state_mut()
        .open_object(tablecraft_ui::ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Table {
                name: "Table1".into(),
            },
        });
    finish_reads(&mut harness);
    harness
        .state_mut()
        .open_object(tablecraft_ui::ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Sql,
        });
    harness.run_steps(2);
    run(&mut harness, "DROP TABLE Table1");
    assert!(harness.state().editing.query_action_review.is_none());
    assert!(harness.state().state.query_action_dialog_open());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    close_target(&mut harness, "Table1");
    let id = reviewed(&mut harness, "DROP TABLE Table1");
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql("SELECT 2".into())
    );
    harness.run_steps(2);
    harness.state_mut().apply_query_action(id);
    assert!(!harness.state().editing.has_save());
    assert_image(&std::fs::read(&path).unwrap(), &original);
    assert!(!journal_path(&path).exists());
}

#[cfg(unix)]
#[test]
fn guarded_reopen_failure_retains_known_after_and_original_sql_without_row_fallback() {
    let before = canonical_blank_creation::source();
    let after = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let (_directory, path, _, mut harness) = opened(before);
    let sql = "CREATE TABLE Items (Id LONG, Score LONG)";
    reviewed(&mut harness, sql);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    assert!(matches!(
        finish_save(&mut harness).outcome,
        SaveOutcome::Saved { .. }
    ));
    // Tamper after the native actor completed but before its distinct checked
    // read. This must not be accepted by a normal path open or row-page fallback.
    let mut foreign = after.clone();
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(
            Instant::now() < deadline,
            "guarded reopen did not terminate"
        );
        harness.run_steps(1);
    }
    assert!(harness.state().state.error.is_some());
    assert!(harness.state().state.native_source_stale());
    assert!(harness.state().editing.query_schema.is_some());
    assert!(matches!(harness.state().pending, PendingOperation::Read));
    assert_eq!(harness.state().state.sql, sql);
    assert_image(&std::fs::read(&path).unwrap(), &foreign);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Run query")
            .accesskit_node()
            .is_disabled()
    );
    assert!(!harness.state().state.can_inspect_query_parameters());
    let generation = harness.state().generation;
    harness.state_mut().inspect_query_parameters();
    harness.run_steps(2);
    assert_eq!(harness.state().generation, generation);
    assert!(matches!(harness.state().pending, PendingOperation::Read));
    assert_eq!(harness.state().state.sql, sql);
    assert_image(&std::fs::read(&path).unwrap(), &foreign);
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness.state_mut().retry_query_schema_reopen(revision + 1);
    assert!(matches!(harness.state().pending, PendingOperation::Read));
    std::fs::write(&path, &after).unwrap();
    harness
        .get_by_role_and_label(Role::Button, "Retry reopening")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().state.native_source_stale());
    assert!(harness.state().editing.query_schema.is_none());
    assert_eq!(harness.state().state.sql, sql);
    assert_image(&std::fs::read(path).unwrap(), &after);
}

#[test]
fn select_into_run_reviews_all_selected_rows_and_creates_one_atomic_populated_table() {
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::populated;
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let literals = std::iter::once(None)
        .chain((1..900).map(Some))
        .collect::<Vec<_>>();
    let before = populated::numbers(&seed, "Items", &literals);
    let expected = populated::numbers(&before, "Archive", &literals);
    creation_journey(
        before,
        expected,
        "SELECT CopiedScore INTO Archive FROM Items ORDER BY CopiedScore",
        "Archive",
        SaveSchemaChange::TableAdded { column_count: 1 },
        Some(900),
    );
}

#[test]
fn filtered_top_run_reviews_and_saves_every_boundary_tie_once() {
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::populated;

    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(
        &seed,
        "Items",
        &[None, Some(2), Some(5), Some(5), Some(9), Some(10)],
    );
    let expected = populated::numbers(&before, "Archive", &[Some(5), Some(5)]);
    creation_journey(
        before,
        expected,
        "SELECT TOP 1 CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
        "Archive",
        SaveSchemaChange::TableAdded { column_count: 1 },
        Some(2),
    );
}

//! A second Create → Table uses the same UI and reviewed native operation owners.
use super::creating_tests::create_table;
use super::properties_tests::replace;
use super::test_support::{
    add_named_field, apply_and_acknowledge, field_label, name_and_confirm, pending_record, review,
    save_schema_and_bind_record, wait_for_bound_record,
};
use crate::workspace::Desktop;
use crate::workspace::test_support::{app_harness, finish_reads};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_ui::{EditTarget, ObjectKind};

#[test]
fn second_table_duplicate_refusal_correction_and_first_record_preserve_old_rows_and_scratch_sql() {
    let (_directory, path, mut harness) = pending_record("table-draft-number", "Score", "17");
    save_schema_and_bind_record(&mut harness, "Table1");
    review(&mut harness);
    apply_and_acknowledge(&mut harness);
    assert!(harness.state().state.can_create_table());
    let original = std::fs::read(&path).unwrap();
    let old_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let old_tab = harness.state().state.active_object().unwrap();
    let old_target = EditTarget {
        document_revision: old_revision,
        table: "Table1".into(),
        column: "Score".into(),
        parent_ordinal: harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .parent_ordinal(0)
            .unwrap(),
    };
    assert!(harness.state().state.can_edit_cell(&old_target));

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(2);
    let sql = "SELECT 17 AS [Unsaved scratch query]";
    replace(&mut harness, Role::MultilineTextInput, "SQL query", sql);
    create_table(&mut harness);
    harness.get_by_role_and_label(Role::Tab, "Table2");
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    add_named_field(&mut harness, "table-draft-short-text", "Name");
    let input = field_label(&harness, "table-draft-record-field", "Name");
    replace(&mut harness, Role::TextInput, &input, "  Ada  ");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Table name")
            .value()
            .as_deref(),
        Some("Table2")
    );
    name_and_confirm(&mut harness, "Table1");
    finish_reads(&mut harness);
    assert!(harness.state().table_creation.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.table_draft_save_open());
    assert!(harness.state().state.has_unsaved_table_drafts());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    name_and_confirm(&mut harness, "Table2");
    wait_for_bound_record(&mut harness, "Table2");
    let schema_bytes = std::fs::read(&path).unwrap();
    assert_ne!(schema_bytes, original);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .value()
            .as_deref(),
        Some("  Ada  ")
    );
    assert_ne!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        old_revision
    );
    assert!(!harness.state().state.can_activate_object(&old_tab));
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    apply_and_acknowledge(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][0].text,
        "1"
    );
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][1].text,
        "  Ada  "
    );
    assert!(!harness.state().state.can_create_table());
    assert!(!journal_path(&path).exists());
    let tabs = harness.state().state.object_tabs();
    let old = tabs
        .iter()
        .find(|tab| matches!(&tab.target.kind, ObjectKind::Table { name } if name == "Table1"))
        .unwrap()
        .clone();
    let scratch = tabs
        .iter()
        .find(|tab| matches!(tab.target.kind, ObjectKind::Sql))
        .unwrap()
        .clone();
    assert_ne!(old.target.document_revision, old_revision);
    harness.state_mut().activate_object(old);
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Table1");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "17");
    assert!(!harness.state().state.can_edit_cell(&old_target));
    harness.state_mut().activate_object(scratch);
    harness.run_steps(2);
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
            .value()
            .as_deref(),
        Some(sql)
    );

    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_eq!(
        reopened
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .tables
            .iter()
            .filter(|table| !table.system)
            .count(),
        2
    );
    assert!(!reopened.state().state.can_create_table());
    reopened
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    reopened.run_steps(2);
    assert!(
        reopened
            .get_by_role_and_label(Role::Button, "Table")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        reopened
            .get_by_role_and_label(Role::Button, "Table Design")
            .accesskit_node()
            .is_disabled()
    );

    // A fresh host must read actual saved records, rather than retain the
    // previous host's rows or merely enumerate the new catalog entry.
    reopened
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    reopened.run_steps(2);
    for (sql, expected) in [
        ("SELECT [Score] FROM [Table1]", "17"),
        ("SELECT [Name] FROM [Table2]", "  Ada  "),
    ] {
        replace(&mut reopened, Role::MultilineTextInput, "SQL query", sql);
        let run = reopened.state().state.localizer.text("query-run");
        reopened
            .get_by_role_and_label(Role::Button, &run)
            .click_accesskit();
        reopened.run_steps(1);
        finish_reads(&mut reopened);

        let result = reopened.state().state.query_data.as_ref().unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0].text, expected);
    }
}

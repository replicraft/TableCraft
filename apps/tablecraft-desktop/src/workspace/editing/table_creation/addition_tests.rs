//! Repeated Create → Table uses the same UI and reviewed native operation owners.
use super::creating_tests::create_table;
use super::properties_tests::replace;
use super::test_support::{
    add_named_field, apply_and_acknowledge, field_label, name_and_confirm, pending_record, review,
    save_schema_and_bind_record, start_save, wait_for_bound_record,
};
use crate::workspace::Desktop;
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_mixed_add_table;
use tablecraft_ui::{EditTarget, ObjectKind};

#[test]
fn repeated_tables_keep_duplicate_correction_old_rows_and_scratch_sql_across_separate_saves() {
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
    assert!(harness.state().state.can_create_table());
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

    for (table, value) in [("Table3", "Grace"), ("Table4", "Linus")] {
        let before = std::fs::read(&path).unwrap();
        create_table(&mut harness);
        harness.get_by_role_and_label(Role::Tab, table);
        assert_eq!(harness.state().state.sql, sql);
        add_named_field(&mut harness, "table-draft-short-text", "Name");
        let input = field_label(&harness, "table-draft-record-field", "Name");
        replace(&mut harness, Role::TextInput, &input, value);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        save_schema_and_bind_record(&mut harness, table);
        let schema = std::fs::read(&path).unwrap();
        assert_ne!(schema, before);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, "Name")
                .value()
                .as_deref(),
            Some(value)
        );
        review(&mut harness);
        assert!(harness.state().editing.insert_review.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), schema);
        apply_and_acknowledge(&mut harness);
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.table, table);
        assert_eq!(data.rows.len(), 1);
        assert_eq!(data.rows[0][0].text, "1");
        assert_eq!(data.rows[0][1].text, value);
        assert_eq!(harness.state().state.sql, sql);
        assert!(harness.state().state.can_create_table());
        assert!(!journal_path(&path).exists());
    }

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
        4
    );
    assert!(reopened.state().state.can_create_table());
    reopened
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    reopened.run_steps(2);
    assert!(
        !reopened
            .get_by_role_and_label(Role::Button, "Table")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        !reopened
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
        ("SELECT [Name] FROM [Table3]", "Grace"),
        ("SELECT [Name] FROM [Table4]", "Linus"),
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

#[test]
fn populated_design_keyed_then_keyless_then_generated_tables_keep_each_separate_record() {
    let (_directory, path, mut harness) = pending_record("table-draft-number", "Score", "17");
    save_schema_and_bind_record(&mut harness, "Original");
    review(&mut harness);
    apply_and_acknowledge(&mut harness);

    for (table, value, keyed) in [("Manual", "  Ada  ", true), ("Keyless", "日本語", false)] {
        assert!(harness.state().state.can_create_table_design());
        let before = std::fs::read(&path).unwrap();
        harness
            .get_by_role_and_label(Role::Tab, "Create")
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Table Design")
            .click_accesskit();
        harness.run_steps(2);
        super::manual_tests::add_field(&mut harness, "table-draft-short-text", "Name");
        super::manual_tests::add_field(&mut harness, "table-draft-number", "ActualKey");
        if keyed {
            harness
                .get_by_role_and_label(Role::Button, "ActualKey")
                .click_accesskit();
            harness.run_steps(2);
            harness
                .get_by_role_and_label(Role::Button, "Primary Key")
                .click_accesskit();
            harness.run_steps(2);
        }
        super::design_tests::view(&mut harness, "ribbon-datasheet");
        for (field, text) in [("Name", value), ("ActualKey", "7")] {
            let label = field_label(&harness, "table-draft-record-field", field);
            replace(&mut harness, Role::TextInput, &label, text);
        }
        start_save(&mut harness, table);
        if !keyed {
            harness.run_steps(2);
            assert_eq!(std::fs::read(&path).unwrap(), before);
            harness
                .get_by_role_and_label(Role::Button, "No")
                .click_accesskit();
            harness.run_steps(2);
        }
        wait_for_bound_record(&mut harness, table);
        let schema = std::fs::read(&path).unwrap();
        // The independently established first user-page boundary excludes only
        // system catalog/allocation pages. All existing user pages are retained.
        let first_user_byte = 7 * 4096;
        assert_eq!(
            &schema[first_user_byte..before.len()],
            &before[first_user_byte..]
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, "Name")
                .value()
                .as_deref(),
            Some(value)
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, "ActualKey")
                .value()
                .as_deref(),
            Some("7")
        );
        review(&mut harness);
        apply_and_acknowledge(&mut harness);
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.table, table);
        assert_eq!(
            data.rows[0]
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>(),
            [value, "7"]
        );
        assert!(harness.state().state.can_create_table());
        assert!(harness.state().state.can_create_table_design());
        assert!(!journal_path(&path).exists());
    }

    create_table(&mut harness);
    add_named_field(&mut harness, "table-draft-short-text", "Name");
    let input = field_label(&harness, "table-draft-record-field", "Name");
    replace(&mut harness, Role::TextInput, &input, "Grace");
    save_schema_and_bind_record(&mut harness, "Generated");
    review(&mut harness);
    apply_and_acknowledge(&mut harness);
    drop(harness);

    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path)));
    finish_reads(&mut fresh);
    fresh
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    fresh.run_steps(2);
    for (sql, expected) in [
        ("SELECT [Score] FROM [Original]", "17"),
        ("SELECT [Name] FROM [Manual]", "  Ada  "),
        ("SELECT [Name] FROM [Keyless]", "日本語"),
        ("SELECT [Name] FROM [Generated]", "Grace"),
    ] {
        replace(&mut fresh, Role::MultilineTextInput, "SQL query", sql);
        let run = fresh.state().state.localizer.text("query-run");
        fresh
            .get_by_role_and_label(Role::Button, &run)
            .click_accesskit();
        fresh.run_steps(1);
        finish_reads(&mut fresh);
        let data = fresh.state().state.query_data.as_ref().unwrap();
        assert_eq!(data.rows.len(), 1);
        assert_eq!(data.rows[0][0].text, expected);
    }
}

#[test]
fn keyed_creation_crosses_the_old_catalog_window_and_retains_both_initial_inputs() {
    let source = canonical_mixed_add_table::capacity_source();
    let (_directory, path, original, mut harness) = opened(source.bytes);
    assert!(harness.state().state.can_create_table());
    assert!(harness.state().state.can_create_table_design());
    harness
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Table Design")
        .click_accesskit();
    harness.run_steps(2);
    super::manual_tests::add_field(&mut harness, "table-draft-short-text", "Name");
    super::manual_tests::add_field(&mut harness, "table-draft-number", "ActualKey");
    harness
        .get_by_role_and_label(Role::Button, "ActualKey")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Primary Key")
        .click_accesskit();
    harness.run_steps(2);
    super::design_tests::view(&mut harness, "ribbon-datasheet");
    for (field, text) in [("Name", "  retained 日本語  "), ("ActualKey", "7")] {
        let input = field_label(&harness, "table-draft-record-field", field);
        replace(&mut harness, Role::TextInput, &input, text);
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);
    save_schema_and_bind_record(&mut harness, "Last");
    let schema = std::fs::read(&path).unwrap();
    assert!(schema.len() > 64 * 4096);
    assert_eq!(&schema[7 * 4096..original.len()], &original[7 * 4096..]);
    assert!(!journal_path(&path).exists());
    for (field, text) in [("Name", "  retained 日本語  "), ("ActualKey", "7")] {
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, field)
                .value()
                .as_deref(),
            Some(text)
        );
    }
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), schema);
    apply_and_acknowledge(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["  retained 日本語  ", "7"]
    );
    assert!(harness.state().state.can_create_table());
    assert!(harness.state().state.can_create_table_design());
    assert!(!journal_path(&path).exists());
    let complete = std::fs::read(&path).unwrap();
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    fresh
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    fresh.run_steps(2);
    replace(
        &mut fresh,
        Role::MultilineTextInput,
        "SQL query",
        "SELECT [Name],[ActualKey] FROM [Last]",
    );
    let run = fresh.state().state.localizer.text("query-run");
    fresh
        .get_by_role_and_label(Role::Button, &run)
        .click_accesskit();
    fresh.run_steps(1);
    finish_reads(&mut fresh);
    let data = fresh.state().state.query_data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(
        data.rows[0]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["  retained 日本語  ", "7"]
    );
    assert_eq!(std::fs::read(&path).unwrap(), complete);
}

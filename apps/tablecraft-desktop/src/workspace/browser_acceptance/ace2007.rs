//! Original ACE2007 sample reads compose with source-observed write refusals.
use crate::workspace::test_support::{finish_reads, opened};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::Path;
use tablecraft_runtime::journal_path;

#[test]
fn original_ace2007_table_and_sql_reads_keep_unsupported_schema_authoring_inert() {
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(&sample).unwrap();
    let (_directory, path, _, mut harness) = opened(original.clone());
    assert!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .format
            .contains("Ace2007")
    );

    harness
        .get_by_role_and_label(Role::TextInput, "Search objects")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "Search objects")
        .type_text("ROULETTE_TB");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "ROULETTE_TB")
        .click();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(&mut harness);
    let table_tab = harness.state().state.active_object().unwrap();
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.table, "ROULETTE_TB");
    let order = page
        .columns
        .iter()
        .position(|column| column.name == "ORDR_NBR")
        .unwrap();
    // The browser preserves physical source order. These literal memberships
    // agree with the existing original-sample native_queries observations;
    // only the explicit SQL ORDER BY below imposes their sorted order.
    assert!(page.rows.iter().any(|row| row[order].text == "1"));
    assert!(page.rows.iter().any(|row| row[order].text == "2"));

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text("SELECT TOP 2 ORDR_NBR FROM ROULETTE_TB ORDER BY ORDR_NBR");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.columns[0].name, "ORDR_NBR");
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0][0].text, "1");
    assert_eq!(result.rows[1][0].text, "2");
    assert!(result.source_revision.is_none());

    harness
        .get_by_role_and_label(Role::Tab, "ROULETTE_TB")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(harness.state().state.active_object(), Some(table_tab));
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Design view")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    let (pending, available, error) = harness
        .state()
        .state
        .field_addition_inspection(&tab)
        .unwrap();
    assert!(!pending);
    assert_eq!(available, Some(false), "{error:?}");
    let fields = harness.state().state.schema.as_ref().unwrap().fields.len();
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "New field")
            .accesskit_node()
            .is_disabled()
    );
    assert!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Data Type")
            .accesskit_node()
            .is_disabled()
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_addition_open());
    assert_eq!(
        harness.state().state.schema.as_ref().unwrap().fields.len(),
        fields
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read(sample).unwrap(), original);
    assert!(!journal_path(&path).exists());
}

//! Actual Number header save followed by an omitted-default GUI record save.
use super::review;
use crate::workspace::Desktop;
use crate::workspace::test_support::{self, finish_reads, finish_save, number_column::add_number};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;
use tablecraft_ui::CellKind;

#[test]
fn saved_number_then_new_row_omits_score_and_generates_id_through_real_widgets() {
    saved_numbers_then_new_row_omits_fields_and_generates_id(&["Score"]);
}

#[test]
fn three_saved_numbers_then_new_row_omits_all_fields_and_generates_id_through_real_widgets() {
    saved_numbers_then_new_row_omits_fields_and_generates_id(&["Score", "Score2", "Third"]);
}

fn saved_numbers_then_new_row_omits_fields_and_generates_id(columns: &[&str]) {
    let before = raw::source(raw::Source::IdOnly);
    let mut after_column = before.clone();
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    for (index, column) in columns.iter().enumerate() {
        after_column = if index == 0 {
            raw::append(&after_column, raw::Source::IdOnly, column)
        } else {
            raw::repeated::append(&after_column, column)
        };
        add_number(&mut harness, column);
        assert_eq!(std::fs::read(&path).unwrap(), after_column);
        assert!(!journal_path(&path).exists());
    }
    assert!(
        !harness.state().editing.has_save(),
        "Number must finish its full-image checked reopen before New"
    );
    assert!(!harness.state().state.number_column_open());
    assert_eq!(std::fs::read(&path).unwrap(), after_column);
    assert!(!journal_path(&path).exists());
    let schema_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    assert_ne!(schema_revision, original_revision);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(
        page.columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        std::iter::once("ID")
            .chain(columns.iter().copied())
            .collect::<Vec<_>>()
    );
    assert!(page.rows.is_empty());

    insert_omitted_fields(&mut harness, &path, &after_column, schema_revision, columns);
}

fn insert_omitted_fields(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    after_column: &[u8],
    schema_revision: u64,
    columns: &[&str],
) {
    // The ordinary Ribbon starts a real source-bound insertion. Neither ID nor
    // any Number field is supplied; no display string becomes an edit value.
    let new = harness.get_by_role_and_label(Role::Button, "New");
    assert!(!new.accesskit_node().is_disabled());
    new.click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    let generated = harness.get_by_role_and_label(Role::Label, "(New)");
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::SetValue)
    );
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "ID")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::MultilineTextInput, "ID")
            .is_none()
    );
    for column in columns {
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, *column)
                .value()
                .as_deref(),
            Some("")
        );
        // Exercise the actual mode menu, then explicitly omit each field. No
        // literal zero, NULL or empty numeric value is submitted.
        omit_field(harness, column);
    }
    review(harness);
    let review_id = harness.state().editing.insert_review.as_ref().unwrap().0;
    assert!(harness.state().state.can_apply_insert(review_id));
    assert_eq!(
        std::fs::read(path).unwrap(),
        after_column,
        "row review is inert"
    );
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()
            && receipt.query_change.is_none()),
        "{report:?}"
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(path).exists());
    let reopened = harness.state().state.data.as_ref().unwrap();
    assert_eq!(reopened.table, "Table1");
    assert_ne!(reopened.source_revision, Some(schema_revision));
    assert_eq!(reopened.rows.len(), 1);
    assert_eq!(
        reopened.rows[0][0].text, "1",
        "the native generator supplies ID"
    );
    assert_eq!(reopened.rows[0].len(), columns.len() + 1);
    for (index, column) in columns.iter().enumerate() {
        assert_eq!(reopened.columns[index + 1].name, *column);
        assert_eq!(
            reopened.rows[0][index + 1].text,
            "0",
            "omitted {column} uses its stored default"
        );
    }
    for cell in &reopened.rows[0] {
        assert!(matches!(cell.kind, CellKind::Scalar { is_null: false, .. }));
    }
}

fn omit_field(harness: &mut Harness<'_, Desktop>, column: &str) {
    let mut arguments = tablecraft_ui::FluentArgs::new();
    arguments.set("field", column);
    let mode_label = harness
        .state()
        .state
        .localizer
        .format("insertion-field-input", &arguments);
    for mode in ["Value", "Omitted"] {
        harness
            .get_by_role_and_label(Role::Button, &mode_label)
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::RadioButton, mode)
            .click_accesskit();
        harness.run_steps(2);
    }
}

#[test]
fn ordinary_number_rollover_then_new_row_omits_all_fourteen_fields() {
    let mut before = raw::repeated::source(raw::Source::IdOnly);
    let mut names = vec![String::from("Score")];
    for number in 2..=13 {
        let name = format!("N{number}");
        before = raw::repeated::append(&before, &name);
        names.push(name);
    }
    let expected = raw::repeated::append(&before, "N14");
    assert_eq!(before.len(), 11 * 4096);
    assert_eq!(expected.len(), 12 * 4096);
    assert_eq!(&before[10 * 4096..], &expected[10 * 4096..11 * 4096]);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    // Ordinary Desktop::new performs source inspection; no flag/budget override
    // or synthetic pre-qualification substitutes for the real native service.
    add_number(&mut harness, "N14");
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.number_column_open());
    let fresh_tab = harness.state().state.active_object().unwrap();
    assert_eq!(fresh_tab.id, original_tab.id);
    assert_ne!(
        fresh_tab.target.document_revision,
        original_tab.target.document_revision
    );
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), 15);
    assert_eq!(page.columns.last().unwrap().name, "N14");
    assert!(page.rows.is_empty());
    names.push(String::from("N14"));
    let columns = names.iter().map(String::as_str).collect::<Vec<_>>();
    insert_omitted_fields(
        &mut harness,
        &path,
        &expected,
        fresh_tab.target.document_revision,
        &columns,
    );
}

//! Actual Number header save followed by an omitted-default GUI record save.
use crate::workspace::test_support::{
    self,
    field_addition::add_field,
    finish_reads, finish_save,
    inserting::{choose_field_mode, insert_omitted_fields, review_insert},
};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::FieldAdditionKind;
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
        add_field(&mut harness, column, FieldAdditionKind::Number);
        assert_eq!(std::fs::read(&path).unwrap(), after_column);
        assert!(!journal_path(&path).exists());
    }
    assert!(
        !harness.state().editing.has_save(),
        "Number must finish its full-image checked reopen before New"
    );
    assert!(!harness.state().state.field_addition_open());
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
    add_field(&mut harness, "N14", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_addition_open());
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

#[test]
fn ordinary_short_text_number_short_text_saves_and_distinct_row_inputs_use_real_widgets() {
    let before = raw::mixed::source(raw::mixed::SourceCase::IdOnly);
    let label = raw::mixed::append_text(&before, "Label", 255, true);
    let number = raw::mixed::append_number(&label, "AfterLabel");
    let other = raw::mixed::append_text(&number, "OtherLabel", 255, true);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    for (kind, name, expected) in [
        (FieldAdditionKind::ShortText, "Label", &label),
        (FieldAdditionKind::Number, "AfterLabel", &number),
        (FieldAdditionKind::ShortText, "OtherLabel", &other),
    ] {
        add_field(&mut harness, name, kind);
        assert_eq!(std::fs::read(&path).unwrap(), *expected);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.field_addition_open());
        assert_eq!(
            harness.state().state.active_object().unwrap().id,
            original_tab.id
        );
    }
    let fields = ["Label", "AfterLabel", "OtherLabel"];
    for (position, modes) in [
        ["Omitted", "Omitted", "Omitted"],
        ["NULL", "NULL", "NULL"],
        ["Value", "Value", "Value"],
    ]
    .into_iter()
    .enumerate()
    {
        let new = harness.get_by_role_and_label(Role::Button, "New");
        assert!(!new.accesskit_node().is_disabled());
        new.click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
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
        for (field, mode) in fields.iter().zip(modes) {
            choose_field_mode(&mut harness, field, mode);
        }
        if position == 2 {
            // Label's explicit Value remains an empty string, distinct from NULL.
            for (field, text) in [("AfterLabel", "37"), ("OtherLabel", "written")] {
                harness
                    .get_by_role_and_label(Role::TextInput, field)
                    .focus();
                harness.run_steps(1);
                harness
                    .get_by_role_and_label(Role::TextInput, field)
                    .type_text(text);
                harness.run_steps(2);
            }
        }
        review_insert(&mut harness);
        let id = harness.state().editing.insert_review.as_ref().unwrap().0;
        assert!(harness.state().state.can_apply_insert(id));
        if position == 0 {
            assert_eq!(std::fs::read(&path).unwrap(), other, "review is inert");
        }
        harness
            .get_by_role_and_label(Role::Button, "Apply new row to original file")
            .click_accesskit();
        harness.run_steps(1);
        let report = finish_save(&mut harness);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()
                && receipt.query_change.is_none()),
            "{report:?}"
        );
        harness
            .get_by_role_and_label(Role::Button, "Close")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(!harness.state().editing.has_save());
        assert!(!journal_path(&path).exists());
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(
            data.columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["ID", "Label", "AfterLabel", "OtherLabel"]
        );
        assert_eq!(data.rows.len(), position + 1);
        let row = &data.rows[position];
        assert_eq!(row[0].text, (position + 1).to_string());
        let expected = match position {
            0 => [("NULL", true), ("0", false), ("NULL", true)],
            1 => [("NULL", true), ("NULL", true), ("NULL", true)],
            2 => [("", false), ("37", false), ("written", false)],
            _ => unreachable!(),
        };
        for (cell, (text, is_null)) in row[1..].iter().zip(expected) {
            assert_eq!(cell.text, text);
            assert!(
                matches!(cell.kind, CellKind::Scalar { is_null: actual, .. } if actual == is_null)
            );
        }
    }
}

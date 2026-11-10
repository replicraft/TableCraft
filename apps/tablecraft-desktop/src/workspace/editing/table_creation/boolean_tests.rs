//! Real Yes/No creation, typed edits and checkbox interaction baselines.
use super::{
    creating_tests::create_table,
    test_support::{add_named_field, initial_table, start_save},
};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::review_and_save,
        finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    boolean::{self as raw, Input},
    fields::{self, Case},
};
use tablecraft_ui::{CellKind, DisplayControl};

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    assert_eq!(actual.len(), expected.len(), "complete Yes/No image length");
    let difference = actual
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (a, b))| a != b);
    assert!(
        difference.is_none(),
        "first Yes/No byte mismatch: {difference:?}"
    );
}

fn create(
    retained: bool,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Harness<'static, Desktop>,
    Vec<u8>,
    u32,
) {
    let (directory, path, mut harness, expected, definition) = if retained {
        let source = fields::source(Case::FirstNumber).bytes;
        let definition = u32::try_from(source.len() / 4096).unwrap();
        let (directory, path, _, mut harness) = opened(source.clone());
        create_table(&mut harness);
        let expected = raw::append_table(&source, "EnabledValues");
        (directory, path, harness, expected, definition)
    } else {
        let (directory, path, harness) = initial_table();
        (directory, path, harness, raw::source(), 7)
    };
    add_named_field(&mut harness, "table-draft-yes-no", "Enabled");
    start_save(
        &mut harness,
        if retained { "EnabledValues" } else { "Table1" },
    );
    finish_reads(&mut harness);
    assert_image(&path, &expected);
    (directory, path, harness, expected, definition)
}

fn insert(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    definition: u32,
    input: Input,
) -> Vec<u8> {
    begin_generated_insert(harness);
    match input {
        Input::Omitted => choose_field_mode(harness, "Enabled", "Omitted"),
        Input::Value(value) => {
            choose_field_mode(harness, "Enabled", "Value");
            harness
                .get_by_role_and_label(Role::TextInput, "Enabled")
                .focus();
            harness.run_steps(1);
            harness
                .get_by_role_and_label(Role::TextInput, "Enabled")
                .type_text(if value { "true" } else { "false" });
            harness.run_steps(2);
        }
    }
    review_and_save_insert(harness, path, before);
    let expected = raw::append(before, definition, input);
    assert_image(path, &expected);
    expected
}

fn context_edit(harness: &mut Harness<'_, Desktop>, row: usize, value: bool) {
    let row_label = format!("row {}:", row + 1);
    let cell = &harness.state().state.data.as_ref().unwrap().rows[row][1];
    let role = match cell.kind {
        CellKind::Scalar {
            display_control: DisplayControl::Native(106),
            ..
        } => Role::CheckBox,
        _ => Role::Label,
    };
    harness
        .get_all_by_role(role)
        .find(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("Enabled,") && label.contains(&row_label))
        })
        .expect("the actual displayed native Boolean cell")
        .click_secondary();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Edit cell…")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().state.edit_dialog_open());
    assert!(
        harness
            .get_by_role_and_label(Role::CheckBox, "Use NULL (subject to validation)")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text(if value { "true" } else { "false" });
    harness.run_steps(2);
}

fn typed_journey(retained: bool) {
    let (_directory, path, mut harness, mut expected, definition) = create(retained);
    expected = insert(&mut harness, &path, &expected, definition, Input::Omitted);
    expected = insert(
        &mut harness,
        &path,
        &expected,
        definition,
        Input::Value(true),
    );
    for (row, value) in [(0, true), (1, false)] {
        context_edit(&mut harness, row, value);
        let after = raw::update(&expected, definition, row, value);
        review_and_save(&mut harness, &path, &expected, &after);
        expected = after;
    }
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let table = if retained { "EnabledValues" } else { "Table1" };
    reopened.get_by_role_and_label(Role::Button, table).focus();
    reopened.run_steps(1);
    reopened.key_press(Key::Enter);
    reopened.run_steps(1);
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, table);
    assert_eq!(data.rows.len(), 2);
    for (row, checked) in data.rows.iter().zip([true, false]) {
        assert!(
            matches!(row[1].kind, CellKind::Scalar { is_null: false, boolean: Some(actual), .. } if actual == checked)
        );
    }
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn initial_yes_no_defaults_and_context_edits_save_exact_boolean_bits() {
    typed_journey(false);
}

#[test]
fn appended_yes_no_defaults_and_context_edits_save_exact_boolean_bits() {
    typed_journey(true);
}

#[test]
fn created_yes_no_checkbox_click_and_space_propose_reviewed_toggles() {
    let (_directory, path, mut harness, expected, definition) = create(false);
    let mut expected = insert(&mut harness, &path, &expected, definition, Input::Omitted);
    for (keyboard, checked) in [(false, true), (true, false)] {
        let value = &harness.state().state.data.as_ref().unwrap().rows[0][1];
        assert!(
            matches!(
                value.kind,
                CellKind::Scalar {
                    stored_calculation: false,
                    display_control: DisplayControl::Native(106),
                    ..
                }
            ),
            "created Yes/No must expose a real checkbox declaration before toggle interaction"
        );
        let checkbox = harness
            .get_all_by_role(Role::CheckBox)
            .find(|node| {
                node.accesskit_node()
                    .label()
                    .is_some_and(|label| label.starts_with("Enabled,"))
            })
            .expect("actual displayed native checkbox");
        if keyboard {
            checkbox.focus();
            harness.run_steps(1);
            harness.key_press(Key::Space);
        } else {
            checkbox.click();
        }
        harness.run_steps(1);
        finish_reads(&mut harness);
        assert!(
            harness.state().state.edit_dialog_open(),
            "checkbox interaction must request a checked edit rather than merely select the cell"
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
                .value()
                .as_deref(),
            Some(if checked { "true" } else { "false" })
        );
        assert_image(&path, &expected);
        let after = raw::update(&expected, definition, 0, checked);
        review_and_save(&mut harness, &path, &expected, &after);
        expected = after;
    }
    assert!(!journal_path(&path).exists());
}

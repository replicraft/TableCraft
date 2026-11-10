//! Actual New-row saves over current sparse declarations and retained rename history.
use crate::workspace::Desktop;
use crate::workspace::test_support::inserting::{
    begin_generated_insert, choose_field_mode, review_and_save_insert,
};
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_core::values::Value;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    current_insert::{self as raw, Input},
    deleted_field::populated::{Case, readdition::Target},
};
use tablecraft_ui::CellKind;

fn expected_cells(values: Vec<Value>) -> Vec<(String, bool)> {
    values
        .into_iter()
        .map(|value| match value {
            Value::Null => ("NULL".into(), true),
            Value::Int32(value) => (value.to_string(), false),
            Value::Text(value) => (value, false),
            Value::Binary(bytes) => {
                let text = if bytes.is_empty() {
                    "0x"
                } else {
                    assert_eq!(bytes, [0, 255, 33]);
                    "0x00ff21"
                };
                (text.into(), false)
            }
            _ => panic!(
                "The independent current INSERT oracle has only LONG, Text and Binary fields"
            ),
        })
        .collect()
}

fn assert_data(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    original: &[Vec<(String, CellKind)>],
    appended: &[Vec<(String, bool)>],
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), columns.len());
    for (field, (name, number)) in page.columns.iter().zip(columns) {
        assert_eq!(&field.name, name);
        assert_eq!(&field.native_number, number);
    }
    assert_eq!(page.rows.len(), original.len() + appended.len());
    for (row, expected) in page.rows.iter().zip(original) {
        assert_eq!(row.len(), expected.len());
        for (cell, (text, kind)) in row.iter().zip(expected) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "INSERT changed an existing cell kind");
        }
    }
    for (row, expected) in page.rows[original.len()..].iter().zip(appended) {
        assert_eq!(row.len(), expected.len());
        for (cell, (text, null)) in row.iter().zip(expected) {
            assert_eq!(&cell.text, text);
            assert!(matches!(cell.kind, CellKind::Scalar { is_null, .. } if is_null == *null));
        }
    }
}

fn value_input(harness: &mut Harness<'_, Desktop>, field: &str, value: &str) {
    choose_field_mode(harness, field, "Value");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, field)
            .value()
            .as_deref(),
        Some("")
    );
    if !value.is_empty() {
        harness
            .get_by_role_and_label(Role::TextInput, field)
            .focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, field)
            .type_text(value);
        harness.run_steps(2);
    }
}

fn journey(case: Case, target: Target, binary: Option<&[u8]>) {
    let before = raw::source(case, target, true, true);
    let (_directory, path, mut previous, mut harness) = opened(before);
    let page = harness.state().state.data.as_ref().unwrap();
    let columns = page
        .columns
        .iter()
        .map(|field| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let original = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let field = raw::name(target, true);
    let mut appended = Vec::new();
    let inputs: &[Input] = if binary.is_some() {
        &[Input::Unicode]
    } else {
        match target {
            Target::Number => &raw::NUMBER_INPUTS,
            Target::ShortText => &raw::TEXT_INPUTS,
        }
    };
    let first_id = if matches!(case, Case::NumberPresent) {
        5
    } else {
        4
    };
    for (position, &input) in inputs.iter().enumerate() {
        let id = first_id + i32::try_from(position).unwrap();
        let expected = if let Some(binary) = binary {
            raw::append_with_binary(&previous, case, target, input, binary)
        } else {
            raw::append(&previous, case, target, input)
        };
        let row = if let Some(binary) = binary {
            raw::row_with_binary(case, target, input, id, binary)
        } else {
            raw::row(case, target, input, id)
        };
        let old_tab = harness.state().state.active_object().unwrap();
        begin_generated_insert(&mut harness);
        for (name, _) in &columns[1..] {
            choose_field_mode(&mut harness, name, "Omitted");
        }
        match input {
            Input::Omitted => {}
            Input::Null => choose_field_mode(&mut harness, field, "NULL"),
            Input::Number123 => value_input(&mut harness, field, "123"),
            Input::Empty => value_input(&mut harness, field, ""),
            Input::Compressed => value_input(&mut harness, field, "new label"),
            Input::Unicode => value_input(&mut harness, field, "日本語🦀"),
        }
        if let Some(binary) = binary {
            let hex = if binary.is_empty() { "" } else { "00ff21" };
            value_input(&mut harness, "OtherLabel", hex);
        }
        review_and_save_insert(&mut harness, &path, &previous);
        let actual = std::fs::read(&path).unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(
            actual.iter().zip(&expected).position(|(a, b)| a != b),
            None,
            "First byte difference for {case:?} {target:?} {input:?}"
        );
        assert_eq!(actual, expected);
        appended.push(expected_cells(row));
        assert_data(&harness, &columns, &original, &appended);
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.target.kind, old_tab.target.kind);
        assert!(fresh.target.document_revision > old_tab.target.document_revision);
        assert!(!harness.state_mut().state.activate_object(&old_tab));
        assert!(!journal_path(&path).exists());
        previous = expected;
    }
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_data(&reopened, &columns, &original, &appended);
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert!(!journal_path(&path).exists());
}

#[test]
fn number_present_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::NumberPresent, Target::Number, None);
}

#[test]
fn number_present_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::NumberPresent, Target::ShortText, None);
}

#[test]
fn number_absent_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::NumberAbsent, Target::Number, None);
}

#[test]
fn number_absent_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::NumberAbsent, Target::ShortText, None);
}

#[test]
fn text_present_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::TextPresent, Target::Number, None);
}

#[test]
fn text_present_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::TextPresent, Target::ShortText, None);
}

#[test]
fn text_absent_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::TextAbsent, Target::Number, None);
}

#[test]
fn text_absent_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::TextAbsent, Target::ShortText, None);
}

#[test]
fn mixed_text_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::MixedText, Target::Number, None);
}

#[test]
fn mixed_text_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::MixedText, Target::ShortText, None);
}

#[test]
fn mixed_binary_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::MixedBinary, Target::Number, None);
}

#[test]
fn mixed_binary_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::MixedBinary, Target::ShortText, None);
}

#[test]
fn id_only_text_current_number_saves_supplied_and_omitted_rows() {
    journey(Case::IdOnlyText, Target::Number, None);
}

#[test]
fn id_only_text_current_text_saves_supplied_and_omitted_rows() {
    journey(Case::IdOnlyText, Target::ShortText, None);
}

#[test]
fn current_sparse_binary_sibling_is_supplied_as_literal_hex() {
    journey(Case::MixedBinary, Target::ShortText, Some(&[0, 255, 33]));
}

#[test]
fn current_sparse_empty_binary_is_a_supplied_value_distinct_from_null() {
    journey(Case::MixedBinary, Target::ShortText, Some(&[]));
}

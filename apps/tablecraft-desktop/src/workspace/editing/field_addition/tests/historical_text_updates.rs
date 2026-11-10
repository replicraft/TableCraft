//! Actual Text edits after ADD retain siblings across repeated native saves.
use crate::workspace::Desktop;
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::test_support::field_addition::add_field;
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui_kittest::Harness;
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text;
use tablecraft_ui::CellKind;

fn assert_contents(
    harness: &Harness<'_, Desktop>,
    siblings: &[Vec<(String, CellKind)>],
    selected: Option<&str>,
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), siblings.len());
    let last = page.columns.len() - 1;
    assert_eq!(page.columns[last].name, "NewLabel");
    for (row, original) in page.rows.iter().zip(siblings) {
        assert_eq!(row.len(), original.len() + 1);
        for (cell, (text, kind)) in row[..last].iter().zip(original) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "Sibling field kind changed");
        }
    }
    for (position, row) in page.rows.iter().enumerate() {
        let expected = if position == 0 { selected } else { None };
        assert_eq!(row[last].text, expected.unwrap_or("NULL"));
        assert!(
            matches!(row[last].kind, CellKind::Scalar { is_null, .. } if is_null == expected.is_none())
        );
    }
}

fn journey([before, after_add]: [Vec<u8>; 2]) {
    let (_directory, path, _, mut harness) = opened(before);
    let siblings = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    add_field(&mut harness, "NewLabel", FieldAdditionKind::ShortText);
    let added = std::fs::read(&path).unwrap();
    assert_eq!(
        added
            .iter()
            .zip(&after_add)
            .position(|(actual, expected)| actual != expected),
        None,
        "Text ADD first byte difference"
    );
    assert_eq!(added, after_add);
    assert_contents(&harness, &siblings, None);

    let mut previous = after_add;
    let mut original_null = true;
    for (position, value) in [
        Some("seed"),
        Some("日本語 🦀 longer replacement text"),
        Some("x"),
        Some(""),
        None,
        Some("refilled 日本語 🦀"),
    ]
    .into_iter()
    .enumerate()
    {
        let column = siblings[0].len();
        let expected = if position == 0 {
            populated_text::after(&previous, 0, value.unwrap())
        } else {
            populated_text::present::after(&previous, 0, column, value)
        };
        begin_replacement(&mut harness, 0, "NewLabel", original_null, value);
        review_and_save(&mut harness, &path, &previous, &expected);
        assert_contents(&harness, &siblings, value);
        assert!(!journal_path(&path).exists());
        original_null = value.is_none();
        previous = expected;
    }
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_contents(&reopened, &siblings, Some("refilled 日本語 🦀"));
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert!(!journal_path(&path).exists());
}

#[test]
fn direct_historical_text_is_filled_replaced_cleared_and_refilled() {
    journey(populated_text::pair(
        "NewLabel",
        &[Some(42), None, Some(-7)],
    ));
}

#[test]
fn mixed_historical_text_retains_other_variable_fields_during_replacement() {
    journey(populated_text::mixed_pair());
}

#[test]
fn forwarded_historical_text_keeps_logical_identity_during_replacement() {
    journey(populated_text::forwarded_pair());
}

#[test]
fn id_only_historical_rows_support_present_text_replacement() {
    journey(populated_text::id_only_pair());
}

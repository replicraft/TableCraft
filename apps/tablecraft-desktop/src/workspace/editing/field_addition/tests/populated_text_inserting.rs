//! Real New-row widgets preserve nullable Text input distinctions after ADD.
use crate::workspace::Desktop;
use crate::workspace::test_support::field_addition::add_field;
use crate::workspace::test_support::inserting::{choose_field_mode, review_and_save_insert};
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text;
use tablecraft_ui::CellKind;

#[test]
fn populated_text_add_new_rows_preserve_omitted_null_empty_and_unicode() {
    let [before, mut expected] = populated_text::pair("Notes", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before);
    add_field(&mut harness, "Notes", FieldAdditionKind::ShortText);
    assert_eq!(std::fs::read(&path).unwrap(), expected);

    for (position, (mode, value)) in [
        ("Omitted", None),
        ("NULL", None),
        ("Value", Some("")),
        ("Value", Some("日本語 🦀")),
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
        choose_field_mode(&mut harness, "Score", "Omitted");
        choose_field_mode(&mut harness, "Notes", mode);
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            harness
                .get_by_role_and_label(Role::TextInput, "Notes")
                .focus();
            harness.run_steps(1);
            harness
                .get_by_role_and_label(Role::TextInput, "Notes")
                .type_text(value);
            harness.run_steps(2);
        }
        review_and_save_insert(&mut harness, &path, &expected);
        expected = populated_text::insertion::append_row(&expected, Some(0), value);
        let actual = std::fs::read(&path).unwrap();
        assert_eq!(
            actual.iter().zip(&expected).position(|(a, b)| a != b),
            None,
            "insert {position}: first byte difference"
        );
        assert_eq!(actual, expected, "insert {position}: complete image");
        assert!(!journal_path(&path).exists());
        let page = harness.state().state.data.as_ref().unwrap();
        assert_eq!(page.rows.len(), position + 4);
        let row = &page.rows[position + 3];
        assert_eq!(row[0].text, (position + 4).to_string());
        assert_eq!(row[1].text, "0");
        assert_eq!(row[2].text, value.unwrap_or("NULL"));
        assert!(
            matches!(row[2].kind, CellKind::Scalar { is_null, .. } if is_null == value.is_none())
        );
    }
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 7);
    assert_eq!(page.columns[2].name, "Notes");
    for (position, expected_text) in ["NULL", "NULL", "NULL", "NULL", "NULL", "", "日本語 🦀"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(page.rows[position][0].text, (position + 1).to_string());
        assert_eq!(page.rows[position][2].text, expected_text);
        assert!(
            matches!(page.rows[position][2].kind, CellKind::Scalar { is_null, .. } if is_null == (position < 5))
        );
    }
    for (position, score) in ["42", "NULL", "-7", "0", "0", "0", "0"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(page.rows[position][1].text, score);
    }
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

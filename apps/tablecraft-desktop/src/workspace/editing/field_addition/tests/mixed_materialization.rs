//! Real review/save retains mixed payloads through offset-table growth.
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::{
    Desktop,
    test_support::{app_harness, field_addition::add_field, finish_reads, opened},
};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_mixed::{self as raw, Case};
use tablecraft_ui::CellKind;

#[test]
fn mixed_number_add_materializes_all_rows_and_reopens_without_reencoding_text_or_binary() {
    for case in [Case::Text, Case::Binary] {
        let [before, after_add] = raw::pair(case);
        let (_directory, path, _, mut harness) = opened(before);
        add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
        assert_eq!(std::fs::read(&path).unwrap(), after_add);
        let originals = harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .map(|row| {
                row[..4]
                    .iter()
                    .map(|cell| (cell.text.clone(), cell.kind))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut original = after_add;
        for row in 0..3 {
            let expected = raw::after(&original, row, 37 + row as i32);
            let replacement = (37 + row).to_string();
            begin_replacement(&mut harness, row as u32, "Bonus", true, Some(&replacement));
            review_and_save(&mut harness, &path, &original, &expected);
            let page = harness.state().state.data.as_ref().unwrap();
            assert_eq!(page.rows.len(), 3);
            for (position, previous) in originals.iter().enumerate() {
                for (column, (text, kind)) in previous.iter().enumerate() {
                    assert_eq!(&page.rows[position][column].text, text);
                    assert!(page.rows[position][column].kind == *kind);
                }
                if position <= row {
                    assert_eq!(page.rows[position][4].text, (37 + position).to_string());
                } else {
                    assert_eq!(page.rows[position][4].text, "NULL");
                    assert!(matches!(
                        page.rows[position][4].kind,
                        CellKind::Scalar { is_null: true, .. }
                    ));
                }
            }
            original = expected;
        }
        let edited = raw::update_number(&original, 0, Some(99));
        begin_replacement(&mut harness, 0, "AfterLabel", false, Some("99"));
        review_and_save(&mut harness, &path, &original, &edited);
        assert_eq!(
            harness.state().state.data.as_ref().unwrap().rows[0][2].text,
            "99"
        );
        drop(harness);

        let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut reopened);
        let page = reopened.state().state.data.as_ref().unwrap();
        for (row, previous) in originals.iter().enumerate() {
            for (column, (text, kind)) in previous.iter().enumerate() {
                if row == 0 && column == 2 {
                    assert_eq!(page.rows[row][column].text, "99");
                } else {
                    assert_eq!(&page.rows[row][column].text, text);
                    assert!(page.rows[row][column].kind == *kind);
                }
            }
            assert_eq!(page.rows[row][4].text, (37 + row).to_string());
        }
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        assert!(!journal_path(&path).exists());
        assert!(!reopened.state().editing.has_save());
    }
}

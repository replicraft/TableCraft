//! Consecutive reviewed saves retain logical IDs after native record growth.
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::{
    Desktop,
    test_support::{app_harness, field_addition::add_field, finish_reads, opened},
};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated::{
    self, materialization,
};
use tablecraft_ui::CellKind;

#[test]
fn added_number_materializes_each_old_row_then_forwarded_score_still_edits_and_reopens() {
    let [before, after_add] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before);
    add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), after_add);
    let mut original = after_add;
    for row in 0..3 {
        let value = (37 + row).to_string();
        let expected = materialization::after(&original, row, 2, 37 + row as i32);
        begin_replacement(&mut harness, row as u32, "Bonus", true, Some(&value));
        review_and_save(&mut harness, &path, &original, &expected);
        let page = harness.state().state.data.as_ref().unwrap();
        assert_eq!(page.rows.len(), 3);
        for position in 0..3 {
            assert_eq!(page.rows[position][0].text, (position + 1).to_string());
            assert_eq!(page.rows[position][1].text, ["42", "NULL", "-7"][position]);
            if position <= row {
                assert_eq!(page.rows[position][2].text, (37 + position).to_string());
                assert!(matches!(
                    page.rows[position][2].kind,
                    CellKind::Scalar { is_null: false, .. }
                ));
            } else {
                assert_eq!(page.rows[position][2].text, "NULL");
                assert!(matches!(
                    page.rows[position][2].kind,
                    CellKind::Scalar { is_null: true, .. }
                ));
            }
        }
        original = expected;
    }
    let edited = populated::update_scores(&original, &[(0, Some(99))]);
    begin_replacement(&mut harness, 0, "Score", false, Some("99"));
    review_and_save(&mut harness, &path, &original, &edited);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][1].text,
        "99"
    );
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 3);
    for (row, score) in ["99", "NULL", "-7"].into_iter().enumerate() {
        assert_eq!(page.rows[row][0].text, (row + 1).to_string());
        assert_eq!(page.rows[row][1].text, score);
        assert_eq!(page.rows[row][2].text, (37 + row).to_string());
    }
    assert_eq!(std::fs::read(&path).unwrap(), edited);
    assert!(!journal_path(&path).exists());
    assert!(!reopened.state().editing.has_save());
}

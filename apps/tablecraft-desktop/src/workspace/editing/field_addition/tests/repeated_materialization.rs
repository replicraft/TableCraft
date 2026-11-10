//! Two real ADD/fill cycles grow one logical row without duplicating it on reopen.
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::{
    Desktop,
    test_support::{app_harness, field_addition::add_field, finish_reads, opened},
};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    populated::{self, materialization},
    populated_mixed::{self as mixed, Case},
};
use tablecraft_ui::CellKind;

fn repeat_add_and_fill(images: [Vec<u8>; 5], original_columns: usize) {
    let [
        before,
        after_first_add,
        after_first_fill,
        after_second_add,
        after_second_fill,
    ] = images;
    let (_directory, path, _, mut harness) = opened(before);
    // NO_CLONE: Keep an independent pre-edit display oracle after both Desktop
    // instances are dropped; borrowing their cells cannot outlive those owners.
    let original_cells = harness
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

    add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), after_first_add);
    begin_replacement(&mut harness, 0, "Bonus", true, Some("37"));
    review_and_save(&mut harness, &path, &after_first_add, &after_first_fill);
    drop(harness);

    // NO_CLONE: The new Desktop owns its open path; retain our path separately
    // for independent source-image and journal checks after its saves.
    let mut harness = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 3);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][original_columns].text,
        "37"
    );
    add_field(&mut harness, "SecondBonus", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), after_second_add);
    for row in &harness.state().state.data.as_ref().unwrap().rows {
        assert_eq!(row[original_columns + 1].text, "NULL");
        assert!(matches!(
            row[original_columns + 1].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
    begin_replacement(&mut harness, 0, "SecondBonus", true, Some("81"));
    review_and_save(&mut harness, &path, &after_second_add, &after_second_fill);
    drop(harness);

    // NO_CLONE: The reopened Desktop owns its open path while the independent
    // final source-image and journal checks below still need our path.
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 3);
    for (row, originals) in page.rows.iter().zip(&original_cells) {
        assert_eq!(row.len(), original_columns + 2);
        for (cell, (text, kind)) in row.iter().zip(originals) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind);
        }
    }
    for (position, row) in page.rows.iter().enumerate() {
        for (column, value) in [(original_columns, "37"), (original_columns + 1, "81")] {
            assert_eq!(row[column].text, if position == 0 { value } else { "NULL" });
            assert!(matches!(
                row[column].kind,
                CellKind::Scalar { is_null, .. } if is_null == (position != 0)
            ));
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), after_second_fill);
    assert!(!journal_path(&path).exists());
    assert!(!reopened.state().editing.has_save());
    assert!(reopened.state().state.error.is_none());
}

#[test]
fn repeated_number_add_fill_saves_the_same_forwarded_fixed_row_and_fresh_reopens() {
    let [before, after_first_add] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let [after_first_fill, after_second_add] = materialization::forwarded::pair();
    let after_second_fill = materialization::forwarded::after_second(&after_second_add, 81);
    repeat_add_and_fill(
        [
            before,
            after_first_add,
            after_first_fill,
            after_second_add,
            after_second_fill,
        ],
        2,
    );
}

#[test]
fn repeated_number_add_fill_saves_forwarded_text_and_binary_rows_without_reencoding() {
    for case in [Case::Text, Case::Binary] {
        let [before, after_first_add] = mixed::pair(case);
        let [after_first_fill, after_second_add] = mixed::forwarded_pair(case);
        let after_second_fill = mixed::forwarded_after(&after_second_add, 81);
        repeat_add_and_fill(
            [
                before,
                after_first_add,
                after_first_fill,
                after_second_add,
                after_second_fill,
            ],
            4,
        );
    }
}

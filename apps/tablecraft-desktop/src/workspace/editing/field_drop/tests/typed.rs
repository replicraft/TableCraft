//! Typed tail deletion reuses actual confirmation, review, save and fresh reads.
use super::{Entry, choose, inspect};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads, finish_schema_save,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values, field_changes, fields, fixed_scalars,
};
use tablecraft_ui::CellKind;

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete typed DROP image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
}

fn open_table(harness: &mut Harness<'_, Desktop>, table: &str) {
    harness.get_by_role_and_label(Role::Button, table).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, table);
}

fn datasheet(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    let label = harness.state().state.localizer.text("ribbon-datasheet");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn drop_tail(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    name: &str,
    entry: Entry,
) -> Vec<u8> {
    let page = harness.state().state.data.as_ref().unwrap();
    let selected = page
        .columns
        .iter()
        .position(|column| column.name == name)
        .unwrap();
    let columns = page
        .columns
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != selected)
        .map(|(_, column)| (column.name.clone(), column.native_number))
        .collect::<Vec<_>>();
    let rows = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter(|(index, _)| *index != selected)
                .map(|(_, cell)| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    inspect(harness, name, entry);
    choose(harness, "field_drop_confirmation", "No");
    assert!(!harness.state().state.field_drop_open());
    assert!(!harness.state().editing.has_save());
    assert_image(path, before);
    assert!(!journal_path(path).exists());
    if matches!(entry, Entry::Design) {
        datasheet(harness);
    }
    inspect(harness, name, entry);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled(),
        "native source inspection did not admit typed tail {name}"
    );
    choose(harness, "field_drop_confirmation", "Yes");
    assert!(harness.state().state.field_drop_dirty());
    assert!(harness.state().editing.save.is_none());
    assert_image(path, before);
    let after = field_changes::drop_tail(before, definition, catalog_root, name);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(harness);
    assert_image(path, &after);
    assert_eq!(
        &after[(definition as usize + 1) * 4096..(definition as usize + 3) * 4096],
        &before[(definition as usize + 1) * 4096..(definition as usize + 3) * 4096]
    );
    if matches!(entry, Entry::Design) {
        datasheet(harness);
    }
    assert_survivors(harness, &columns, &rows);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(!journal_path(path).exists());
    after
}

fn assert_survivors(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    rows: &[Vec<(String, CellKind)>],
) {
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(
        data.columns
            .iter()
            .map(|column| (column.name.clone(), column.native_number))
            .collect::<Vec<_>>(),
        columns
    );
    assert_eq!(data.rows.len(), rows.len());
    for (row, expected) in data.rows.iter().zip(rows) {
        assert_eq!(row.len(), expected.len());
        for (cell, (text, kind)) in row.iter().zip(expected) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind);
        }
    }
}

#[test]
fn populated_fixed_tail_design_drop_cancels_then_preserves_editable_survivors() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let created = fixed_scalars::append_table(&seed, "Measurements");
    let values = [
        fixed_scalars::Scalar::Byte(17),
        fixed_scalars::Scalar::Int16(-1234),
        fixed_scalars::Scalar::Int64(9_007_199_254_740_993),
        fixed_scalars::Scalar::SingleBits(0x3fa0_0000),
        fixed_scalars::Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
        fixed_scalars::Scalar::DateTimeBits(0x4004_0000_0000_0000),
    ];
    let before = fixed_scalars::append_values(&created, definition, &values);
    let (_directory, path, _, mut harness) = opened(before.clone());
    open_table(&mut harness, "Measurements");
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let dropped = drop_tail(
        &mut harness,
        &path,
        &before,
        definition,
        catalog_root,
        "DateValue",
        Entry::Design,
    );
    begin_replacement(&mut harness, 0, "Score", false, Some("124"));
    let after = field_changes::update_score(&dropped, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &dropped, &after);
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, "Measurements");
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns.len(), 7);
    assert_eq!(data.columns.last().unwrap().name, "DoubleValue");
    assert_eq!(data.rows[0][1].text, "124");
    assert_image(&path, &after);
    assert!(!journal_path(&path).exists());
}

#[test]
fn decimal_then_guid_tail_drop_preserves_raw_history_and_live_number_identity() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let created = exact_values::append_table(&seed, "ExactValues");
    let token = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let before = exact_values::append(
        &created,
        definition,
        Some(token),
        Some(90_071_992_547_409_931_234),
    );
    let (_directory, path, _, mut harness) = opened(before.clone());
    open_table(&mut harness, "ExactValues");
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let decimal = drop_tail(
        &mut harness,
        &path,
        &before,
        definition,
        catalog_root,
        "Amount",
        Entry::Datasheet,
    );
    let guid = drop_tail(
        &mut harness,
        &path,
        &decimal,
        definition,
        catalog_root,
        "Token",
        Entry::Datasheet,
    );
    begin_replacement(&mut harness, 0, "Score", false, Some("124"));
    let after = field_changes::update_score(&guid, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &guid, &after);
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, "ExactValues");
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns.len(), 2);
    assert_eq!(data.columns[0].native_number, Some(0));
    assert_eq!(data.columns[1].native_number, Some(1));
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "124");
    assert_image(&path, &after);
    assert!(!journal_path(&path).exists());
}

//! Actual middle-field context menu, confirmation and Save preserve survivor tabs.
use super::{Entry, choose, inspect};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness, finish_reads, finish_schema_save,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};
use tablecraft_ui::CellKind;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn open_table(harness: &mut Harness<'_, Desktop>, table: &str) {
    harness.get_by_role_and_label(Role::Button, table).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, table);
    if harness.state().state.data.as_ref().unwrap().offset != 0 {
        harness
            .get_by_role_and_label(Role::Button, "First")
            .click_accesskit();
        harness.run_steps(1);
        finish_reads(harness);
    }
}

fn assert_survivors(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    rows: &[Vec<(String, CellKind)>],
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(
        page.columns
            .iter()
            .map(|field| (field.name.clone(), field.native_number))
            .collect::<Vec<_>>(),
        columns
    );
    assert_eq!(page.rows.len(), rows.len());
    for (actual, expected) in page.rows.iter().zip(rows) {
        assert_eq!(actual.len(), expected.len());
        for (cell, (text, kind)) in actual.iter().zip(expected) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "surviving native field kind changed");
        }
    }
}

fn source(case: fields::Case) -> fields::Source {
    let mut source = fields::source(case);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    let before = if matches!(case, fields::Case::FirstNumber) {
        typed_fields::update_currency(&before, source.definition, 0, Some(12345))
    } else {
        before
    };
    source.bytes = before;
    source
}

fn journey(source: fields::Source, table: &str, column: &str) {
    let before = source.bytes;
    let after = field_changes::drop_field(&before, source.definition, source.catalog_root, column);
    let (_directory, path, _, mut harness) = opened(before.clone());
    open_table(&mut harness, "Other");
    let sibling = harness.state().state.active_object().unwrap();
    open_table(&mut harness, table);
    let original = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original);
    let tabs = harness.state().state.object_tabs();
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), source.roots.len());
    let selected = data
        .columns
        .iter()
        .position(|field| field.name == column)
        .unwrap();
    let columns = data
        .columns
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != selected)
        .map(|(_, field)| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let rows = data
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
    inspect(&mut harness, column, Entry::Datasheet);
    choose(&mut harness, "field_drop_confirmation", "No");
    assert!(!harness.state().state.field_drop_open());
    assert!(!harness.state().editing.has_save());
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert!(!journal_path(&path).exists());
    inspect(&mut harness, column, Entry::Datasheet);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled(),
        "actual non-tail source inspection must enable confirmation"
    );
    choose(&mut harness, "field_drop_confirmation", "Yes");
    assert!(harness.state().state.field_drop_dirty());
    assert!(harness.state().editing.save.is_none());
    assert_image(&std::fs::read(&path).unwrap(), &before);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_image(&std::fs::read(&path).unwrap(), &after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.id, original.id);
    assert!(fresh.target.document_revision > original.target.document_revision);
    assert!(!harness.state_mut().state.activate_object(&original));
    let current_tabs = harness.state().state.object_tabs();
    assert_eq!(tabs.len(), current_tabs.len());
    for (old, current) in tabs.iter().zip(&current_tabs) {
        assert_eq!(old.id, current.id);
        assert_eq!(old.target.kind, current.target.kind);
    }
    assert!(current_tabs.iter().any(|tab| tab.id == sibling.id));
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert_survivors(&harness, &columns, &rows);
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    open_table(&mut reopened, table);
    assert_survivors(&reopened, &columns, &rows);
    assert_image(&std::fs::read(&path).unwrap(), &after);
}

#[test]
fn populated_middle_number_context_drop_keeps_currency_and_sibling_tabs() {
    journey(source(fields::Case::FirstNumber), "Table1", "Score");
}

#[test]
fn populated_middle_text_context_drop_preserves_forwarded_rows_and_native_numbers() {
    journey(source(fields::Case::SecondText), "Table1", "Name");
}

#[test]
fn keyless_ordinary_native_zero_context_drop_keeps_score_native_one() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let bytes = field_changes::keyless_first_field(&seed, "Keyless");
    journey(
        fields::Source {
            bytes,
            definition,
            catalog_root,
            roots: vec![0],
        },
        "Keyless",
        "Id",
    );
}

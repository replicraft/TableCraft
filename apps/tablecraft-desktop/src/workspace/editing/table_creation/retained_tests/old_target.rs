//! Creating a sibling must not strand a previously insertable sparse Text table.
use super::{draft, save_schema};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness, finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_insert::{self as old, Input},
        deleted_field::populated::{Case, readdition::Target},
    },
    canonical_retained_add_table::{self as appended, Target as Added},
};
use tablecraft_ui::{CellKind, ObjectKind};

#[test]
fn old_sparse_text_table_still_inserts_after_saving_a_new_sibling() {
    let before = old::source(Case::TextPresent, Target::ShortText, true, true);
    // The row and sibling-table operations touch independently owned storage.
    // This oracle intentionally inserts while the source still has eleven pages,
    // then appends the sibling; the GUI performs them in the opposite order.
    let inserted = old::append(
        &before,
        Case::TextPresent,
        Target::ShortText,
        Input::Unicode,
    );
    let expected = appended::desktop::append(&inserted, "More", Added::GeneratedNumber);
    let (_directory, path, _, mut harness) = opened(before);
    let original = harness.state().state.active_object().unwrap();
    let original_rows = harness
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
    scratch(&mut harness, &original);
    let before_schema = std::fs::read(&path).unwrap();
    draft(&mut harness, Added::GeneratedNumber);
    let schema = save_schema(
        &mut harness,
        &path,
        &before_schema,
        "More",
        Added::GeneratedNumber,
    );
    assert_eq!(schema.len(), 14 * 4096);
    assert!(!harness.state().state.can_activate_object(&original));
    harness
        .get_by_role_and_label(Role::Tab, "Table1")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, "Table1");
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows.len(),
        original_rows.len()
    );
    begin_generated_insert(&mut harness);
    choose_field_mode(&mut harness, "Score", "Omitted");
    choose_field_mode(&mut harness, "CurrentField", "Value");
    harness
        .get_by_role_and_label(Role::TextInput, "CurrentField")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "CurrentField")
        .type_text("日本語🦀");
    harness.run_steps(2);
    review_and_save_insert(&mut harness, &path, &schema);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Table1");
    assert_eq!(
        data.columns
            .iter()
            .map(|column| (column.name.as_str(), column.native_number))
            .collect::<Vec<_>>(),
        [
            ("Id", Some(0)),
            ("Score", Some(1)),
            ("CurrentField", Some(3))
        ]
    );
    assert_eq!(data.rows.len(), original_rows.len() + 1);
    for (row, original) in data.rows.iter().zip(&original_rows) {
        assert_eq!(row.len(), original.len());
        for (cell, (text, kind)) in row.iter().zip(original) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind);
        }
    }
    let row = data.rows.last().unwrap();
    assert_eq!(
        row.iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["4", "0", "日本語🦀"]
    );
    assert!(
        row.iter()
            .all(|cell| matches!(cell.kind, CellKind::Scalar { is_null: false, .. }))
    );
    let sql = harness
        .state()
        .state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.target.kind == ObjectKind::Sql)
        .unwrap();
    harness.state_mut().activate_object(sql);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Table1");
    assert_eq!(data.rows.len(), 4);
    assert_eq!(data.rows[3][2].text, "日本語🦀");
    let navigation = reopened
        .get_all_by_role_and_label(Role::Button, "More")
        .filter(|node| !node.accesskit_node().is_disabled())
        .collect::<Vec<_>>();
    assert_eq!(navigation.len(), 1);
    navigation[0].focus();
    drop(navigation);
    reopened.run_steps(1);
    reopened.key_press(Key::Enter);
    reopened.run_steps(1);
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "More");
    assert!(data.rows.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

//! Initial inline properties are a real rename source without a preparatory ADD.
use crate::workspace::{
    Desktop,
    test_support::{app_harness, field_rename::rename_and_save, finish_reads, opened},
};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes as changes,
    fields::{self, Case},
};

#[test]
fn retained_inline_companion_rename_saves_without_first_adding_a_field() {
    for case in fields::CASES {
        let source = fields::source(case);
        let old = if matches!(case, Case::SecondText) {
            "Name"
        } else {
            "Score"
        };
        let expected = changes::rename(
            &source.bytes,
            source.definition,
            source.catalog_root,
            old,
            "InitialRenamed",
        );
        let (_directory, path, before, mut harness) = opened(source.bytes);
        harness
            .get_by_role_and_label(Role::Button, "Table1")
            .focus();
        harness.run_steps(1);
        harness.key_press(Key::Enter);
        harness.run_steps(1);
        finish_reads(&mut harness);
        let original = harness
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
        rename_and_save(
            &mut harness,
            &path,
            &before,
            &expected,
            old,
            "InitialRenamed",
        );
        drop(harness);
        let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut fresh);
        fresh.get_by_role_and_label(Role::Button, "Table1").focus();
        fresh.run_steps(1);
        fresh.key_press(Key::Enter);
        fresh.run_steps(1);
        finish_reads(&mut fresh);
        let page = fresh.state().state.data.as_ref().unwrap();
        assert!(
            page.columns
                .iter()
                .any(|field| field.name == "InitialRenamed")
        );
        assert_eq!(page.rows.len(), original.len());
        for (row, old) in page.rows.iter().zip(&original) {
            assert_eq!(row.len(), old.len());
            for (cell, (text, kind)) in row.iter().zip(old) {
                assert_eq!(&cell.text, text);
                assert!(cell.kind == *kind);
            }
        }
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
    }
}

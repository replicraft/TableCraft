//! Actual nullable ADD keeps historical rows NULL through fill and later INSERT.
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        field_addition::add_field,
        finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_core::values::Value;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields::{self as raw, Case, Target},
    rows::Input,
};
use tablecraft_ui::CellKind;

fn open_table(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Table1")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, "Table1");
}

fn journey(case: Case) {
    for target in [Target::Number, Target::ShortText] {
        let source = raw::source(case);
        let expected_add = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            target,
        );
        let (_directory, path, _, mut harness) = opened(source.bytes);
        open_table(&mut harness);
        let original = harness.state().state.active_object().unwrap();
        scratch(&mut harness, &original);
        let count = harness.state().state.data.as_ref().unwrap().rows.len();
        add_field(
            &mut harness,
            target.name(),
            match target {
                Target::Number => FieldAdditionKind::Number,
                Target::ShortText => FieldAdditionKind::ShortText,
            },
        );
        assert_eq!(std::fs::read(&path).unwrap(), expected_add);
        let page = harness.state().state.data.as_ref().unwrap();
        assert_eq!(page.rows.len(), count);
        for row in &page.rows {
            assert_eq!(row.last().unwrap().text, "NULL");
            assert!(matches!(
                row.last().unwrap().kind,
                CellKind::Scalar { is_null: true, .. }
            ));
        }
        let (value, input) = match target {
            Target::Number => (Value::Int32(123), "123"),
            Target::ShortText => (Value::Text("日本語🦀".into()), "日本語🦀"),
        };
        let filled = raw::fill(
            &expected_add,
            source.definition,
            source.roots[0],
            target,
            Some(value),
        );
        begin_replacement(&mut harness, 0, target.name(), true, Some(input));
        review_and_save(&mut harness, &path, &expected_add, &filled);
        let mut previous = filled;
        let columns = harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .columns
            .iter()
            .skip(1)
            .map(|field| field.name.clone())
            .collect::<Vec<_>>();
        for supplied_null in [false, true] {
            let expected = raw::append(
                &previous,
                source.definition,
                target,
                if supplied_null {
                    Input::Null
                } else {
                    Input::Omitted
                },
            );
            begin_generated_insert(&mut harness);
            for column in &columns {
                choose_field_mode(
                    &mut harness,
                    column,
                    if supplied_null && column == target.name() {
                        "NULL"
                    } else {
                        "Omitted"
                    },
                );
            }
            review_and_save_insert(&mut harness, &path, &previous);
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
            previous = expected;
        }
        assert!(!journal_path(&path).exists());
        drop(harness);
        let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut fresh);
        open_table(&mut fresh);
        let page = fresh.state().state.data.as_ref().unwrap();
        assert_eq!(page.rows.len(), count + 2);
        assert_eq!(page.rows[0].last().unwrap().text, input);
        assert!(page.rows[count - 1].last().unwrap().text == "NULL");
        assert_eq!(
            page.rows[count].last().unwrap().text,
            match target {
                Target::Number => "0",
                Target::ShortText => "NULL",
            }
        );
        assert_eq!(page.rows[count + 1].last().unwrap().text, "NULL");
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn retained_first_number_table_add_fill_and_insert_preserve_live_sibling() {
    journey(Case::FirstNumber);
}

#[test]
fn retained_second_text_table_long_roots_add_fill_and_insert_preserve_history() {
    journey(Case::SecondText);
}

#[test]
fn original_sparse_table_add_fill_and_insert_work_after_sibling_append() {
    journey(Case::OriginalSparse);
}

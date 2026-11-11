//! The saved-field gallery must expose working ordinary type declarations.
use crate::workspace::test_support::opened;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

#[test]
fn populated_retained_saved_field_gallery_exposes_currency_as_an_actual_declaration() {
    let source = fields::source(fields::Case::FirstNumber);
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let tab = harness.state().state.active_object().unwrap();
    assert_eq!(
        harness
            .state()
            .state
            .field_addition_inspection(&tab)
            .unwrap()
            .1,
        Some(true)
    );
    assert!(
        harness
            .state()
            .state
            .can_name_field_addition(&tab, FieldAdditionKind::Number)
    );
    assert!(!harness.state().state.data.as_ref().unwrap().rows.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    let number = harness.state().state.localizer.text("table-draft-number");
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, &number)
            .accesskit_node()
            .is_disabled()
    );
    let currency = harness.state().state.localizer.text("table-draft-currency");
    let choice = harness.get_by_role_and_label(Role::Button, &currency);
    assert!(
        !choice.accesskit_node().is_disabled(),
        "Currency remains unavailable in the saved-field gallery despite a qualified populated source"
    );
    choice.click_accesskit();
    harness.run_steps(2);
    let name = harness.get_by_role_and_label(Role::TextInput, "Field name");
    assert!(!name.accesskit_node().is_disabled() && !name.accesskit_node().is_read_only());
    assert_image(&path, &before);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
}

#[test]
fn currency_add_preserves_historical_null_then_fills_and_inserts_the_type_default() {
    use crate::workspace::{
        Desktop,
        test_support::{
            app_harness,
            cell_edit::{begin_replacement, review_and_save},
            field_addition::add_field,
            finish_reads,
            inserting::{begin_generated_insert, review_and_save_insert},
            objects::{EXACT_SCRATCH, scratch},
        },
    };
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::typed_fields::{
        self, Kind,
    };
    use tablecraft_ui::CellKind;

    let source = fields::source(fields::Case::FirstNumber);
    let added = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        Kind::Currency,
    );
    let filled =
        typed_fields::update_currency(&added, source.definition, source.roots[0], Some(12_345));
    let inserted = typed_fields::append_omitted(&filled, source.definition, Kind::Currency);
    let (_directory, path, _, mut harness) = opened(source.bytes);
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let count = harness.state().state.data.as_ref().unwrap().rows.len();
    add_field(&mut harness, "Amount", FieldAdditionKind::Currency);
    assert_image(&path, &added);
    for row in &harness.state().state.data.as_ref().unwrap().rows {
        assert!(matches!(
            row.last().unwrap().kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
    begin_replacement(&mut harness, 0, "Amount", true, Some("1.2345"));
    review_and_save(&mut harness, &path, &added, &filled);
    begin_generated_insert(&mut harness);
    review_and_save_insert(&mut harness, &path, &filled);
    assert_image(&path, &inserted);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    fresh.get_by_role_and_label(Role::Button, "Table1").focus();
    fresh.run_steps(1);
    fresh.key_press(egui::Key::Enter);
    fresh.run_steps(1);
    finish_reads(&mut fresh);
    let page = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(page.table, "Table1");
    assert_eq!(page.rows.len(), count + 1);
    assert!(matches!(
        page.rows[1].last().unwrap().kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert!(!matches!(
        page.rows[count].last().unwrap().kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert_image(&path, &inserted);
    assert!(!journal_path(&path).exists());
}

fn assert_image(path: &std::path::Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let first = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && first.is_none(),
        "complete image differs: actual length {}, expected length {}, first differing byte {:?}",
        actual.len(),
        expected.len(),
        first
    );
}

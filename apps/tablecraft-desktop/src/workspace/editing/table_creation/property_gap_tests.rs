//! A promoted catalog-property map must not disable the new table's row save.
use super::{
    creating_tests::create_table,
    design_tests::view,
    properties_tests::replace,
    test_support::{
        add_named_field, apply_and_acknowledge, field_label, review, save_schema_and_bind_record,
    },
};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness, finish_reads,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::rows::{self, Input, Profile},
    retained_catalog_gap,
};
use tablecraft_ui::FieldPropertyValue;

#[test]
fn promoted_catalog_property_map_keeps_described_table_first_row_review_and_save_available() {
    let before = retained_catalog_gap::source();
    let definition = (before.len() / 4096) as u32;
    let (_directory, path, original, mut harness) = opened(before);
    let old_tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &old_tab);
    create_table(&mut harness);
    add_named_field(&mut harness, "table-draft-number", "Score");
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(&mut harness, "ribbon-table-design");
    harness
        .get_by_role_and_label(Role::Button, "Score")
        .click_accesskit();
    harness.run_steps(2);
    let description = "D".repeat(255);
    let label = field_label(&harness, "table-draft-design-description", "Score");
    replace(&mut harness, Role::MultilineTextInput, &label, &description);
    view(&mut harness, "ribbon-datasheet");
    let label = field_label(&harness, "table-draft-record-field", "Score");
    replace(&mut harness, Role::TextInput, &label, "123");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    save_schema_and_bind_record(&mut harness, "Described");
    let schema = std::fs::read(&path).unwrap();
    // No independently authored DDL golden is claimed here. Existing user pages
    // and public property observation independently constrain this schema save.
    assert_eq!(&schema[7 * 4096..original.len()], &original[7 * 4096..]);
    let mut observer = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut observer);
    observer.state_mut().view_object_schema("Described".into());
    finish_reads(&mut observer);
    let fields = &observer.state().state.schema.as_ref().unwrap().fields;
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].name, "Score");
    let saved_description = fields[1]
        .properties
        .iter()
        .find(|property| property.name == "Description")
        .unwrap();
    assert!(
        matches!(&saved_description.value, FieldPropertyValue::Text(value) if value == &description)
    );
    drop(observer);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Score")
            .value()
            .as_deref(),
        Some("123")
    );
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    review(&mut harness);
    assert!(
        harness.state().editing.insert_review.is_some(),
        "new row review refused: {:?}",
        harness.state().state.error
    );
    assert_eq!(std::fs::read(&path).unwrap(), schema);
    assert!(!journal_path(&path).exists());
    apply_and_acknowledge(&mut harness);
    let expected = rows::append(
        &schema,
        definition,
        Profile::DesktopNumber,
        Input::Number(123),
    );
    let actual = std::fs::read(&path).unwrap();
    let difference = actual
        .iter()
        .zip(&expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete row-save image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    fresh
        .get_by_role_and_label(Role::Button, "Described")
        .focus();
    fresh.run_steps(1);
    fresh.key_press(egui::Key::Enter);
    fresh.run_steps(1);
    finish_reads(&mut fresh);
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Described");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "123");
    assert_eq!(std::fs::read(&path).unwrap(), expected);
}

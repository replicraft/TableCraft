//! Exact GUID and Decimal input stays distinct from formatted cell display.
use super::{
    creating_tests::create_table,
    design_tests::view,
    properties_tests::{choose, replace},
    test_support::{
        add_named_field, apply_and_acknowledge, field_label, reveal_field, review,
        save_schema_and_bind_record,
    },
};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values as raw, fields,
};

const INITIAL_TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const REPLACEMENT_TOKEN: [u8; 16] = [
    0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
];
const INITIAL_GUID: &str = "00112233-4455-6677-8899-aabbccddeeff";
const REPLACEMENT_GUID: &str = "fedcba98-7654-3210-0123-456789abcdef";
const INITIAL_AMOUNT: i128 = 90_071_992_547_409_931_234;
const REPLACEMENT_AMOUNT: i128 = -90_071_992_547_409_934_321;

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
}

#[test]
fn retained_guid_and_decimal_cells_preserve_precise_input_nulls_and_refills() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let schema = raw::append_table(&before, "ExactValues");
    let (_directory, path, original, mut harness) = opened(before);
    let old = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &old);
    create_table(&mut harness);
    for name in ["Score", "Token", "Amount"] {
        add_named_field(&mut harness, "table-draft-number", name);
    }
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(&mut harness, "ribbon-table-design");
    harness
        .get_by_role_and_label(Role::Button, "Token")
        .click_accesskit();
    harness.run_steps(2);
    choose(
        &mut harness,
        "design-field-size",
        "table-draft-number-replication-id",
    );
    harness
        .get_by_role_and_label(Role::Button, "Amount")
        .click_accesskit();
    harness.run_steps(2);
    choose(
        &mut harness,
        "design-field-size",
        "table-draft-number-decimal",
    );
    replace(&mut harness, Role::TextInput, "Precision", "28");
    replace(&mut harness, Role::TextInput, "Scale", "4");
    view(&mut harness, "ribbon-datasheet");
    for (field, text) in [("Token", INITIAL_GUID), ("Amount", "9007199254740993.1234")] {
        reveal_field(&mut harness, field, false);
        let label = field_label(&harness, "table-draft-record-field", field);
        replace(&mut harness, Role::TextInput, &label, text);
    }
    assert_image(&path, &original);
    save_schema_and_bind_record(&mut harness, "ExactValues");
    assert_image(&path, &schema);
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_image(&path, &schema);
    apply_and_acknowledge(&mut harness);
    let mut expected = raw::append(
        &schema,
        definition,
        Some(INITIAL_TOKEN),
        Some(INITIAL_AMOUNT),
    );
    assert_image(&path, &expected);
    // Qualify the established LONG service on the exact same current table.
    reveal_field(&mut harness, "Score", false);
    begin_replacement(&mut harness, 0, "Score", false, Some("124"));
    let after = raw::update_score(&expected, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    for (was_null, value, input) in [
        (false, Some(REPLACEMENT_TOKEN), Some(REPLACEMENT_GUID)),
        (false, None, None),
        (true, Some(INITIAL_TOKEN), Some(INITIAL_GUID)),
    ] {
        reveal_field(&mut harness, "Token", false);
        begin_replacement(&mut harness, 0, "Token", was_null, input);
        let after = raw::update_guid(&expected, definition, 0, value);
        review_and_save(&mut harness, &path, &expected, &after);
        expected = after;
    }
    for (was_null, value, input) in [
        (
            false,
            Some(REPLACEMENT_AMOUNT),
            Some("-9007199254740993.4321"),
        ),
        (false, None, None),
        (true, Some(INITIAL_AMOUNT), Some("9007199254740993.1234")),
    ] {
        reveal_field(&mut harness, "Amount", false);
        begin_replacement(&mut harness, 0, "Amount", was_null, input);
        let after = raw::update_decimal(&expected, definition, 0, value);
        review_and_save(&mut harness, &path, &expected, &after);
        expected = after;
    }
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    fresh
        .get_by_role_and_label(Role::Button, "ExactValues")
        .focus();
    fresh.run_steps(1);
    fresh.key_press(egui::Key::Enter);
    fresh.run_steps(1);
    finish_reads(&mut fresh);
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "ExactValues");
    assert_eq!(data.columns.len(), 4);
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "124");
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

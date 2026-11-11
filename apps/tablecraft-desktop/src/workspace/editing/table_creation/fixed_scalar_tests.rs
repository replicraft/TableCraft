//! Typed fixed scalars retain exact bytes across real schema, insert and cell saves.
use super::{
    creating_tests::create_table,
    design_tests::view,
    properties_tests::{choose, replace},
    test_support::{
        add_named_field, apply_and_acknowledge, field_label, review, save_schema_and_bind_record,
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
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields,
    fixed_scalars::{self as raw, Column, Scalar},
};

const INITIAL: [Scalar; 6] = [
    Scalar::Byte(17),
    Scalar::Int16(-1234),
    Scalar::Int64(9_007_199_254_740_993),
    Scalar::SingleBits(0x3fa0_0000),
    Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
    Scalar::DateTimeBits(0x4004_0000_0000_0000),
];
const INPUT: [&str; 6] = [
    "17",
    "-1234",
    "9007199254740993",
    "1.25",
    "1.23456789012345",
    "1900-01-01T12:00:00",
];
const REPLACEMENTS: [(Column, Scalar, &str); 6] = [
    (Column::ByteValue, Scalar::Byte(200), "200"),
    (Column::ShortValue, Scalar::Int16(-2345), "-2345"),
    (
        Column::LargeValue,
        Scalar::Int64(9_007_199_254_740_995),
        "9007199254740995",
    ),
    (Column::SingleValue, Scalar::SingleBits(0xc020_0000), "-2.5"),
    (
        Column::DoubleValue,
        Scalar::DoubleBits(0xc023_c0ca_458a_05ab),
        "-9.87654321012345",
    ),
    (
        Column::DateValue,
        Scalar::DateTimeBits(0x40e6_24e8_aaaa_aaab),
        "2024-02-29T06:30:00",
    ),
];

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

fn reveal(harness: &mut Harness<'_, Desktop>, field: &str, right: bool) {
    harness.get_by_role_and_label(Role::Tab, "Home").focus();
    harness.run_steps(2);
    let viewport = harness.ctx.content_rect();
    let header = harness
        .get_all_by_role(Role::ColumnHeader)
        .map(|node| node.rect().intersect(viewport))
        // Offscreen table headers can extend behind the navigation pane. A
        // header in the right half of the host is inside the actual document.
        .filter(|rect| {
            rect.width() > 40.0 && rect.height() > 5.0 && rect.center().x > viewport.center().x
        })
        .max_by(|left, right| left.center().x.total_cmp(&right.center().x))
        .expect("a real visible field header owns horizontal scrolling");
    harness.event(egui::Event::PointerMoved(header.center()));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(if right { -10_000.0 } else { 10_000.0 }, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(10);
    // Draft headers carry their label directly. Persisted datasheets expose
    // the same name on a child Label of the ColumnHeader container.
    let target = harness.get_all_by_role(Role::ColumnHeader).find(|header| {
        header.accesskit_node().label().as_deref() == Some(field)
            || header.get_all_by_role(Role::Label).any(|label| {
                label.value().as_deref() == Some(field)
                    || label.accesskit_node().label().as_deref() == Some(field)
            })
    });
    let Some(target) = target else {
        let names = harness
            .get_all_by_role(Role::ColumnHeader)
            .flat_map(|header| {
                header
                    .get_all_by_role(Role::Label)
                    .filter_map(|label| label.value())
                    .collect::<Vec<_>>()
            })
            .take(8)
            .collect::<Vec<_>>();
        panic!("requested field header {field} missing; current names: {names:?}");
    };
    assert!(
        target.rect().intersects(viewport),
        "field header {field} is still outside the viewport"
    );
}

#[test]
fn retained_fixed_scalar_cells_keep_typed_precision_nulls_and_refills_after_actual_creation() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (before.len() / 4096) as u32;
    let schema = raw::append_table(&before, "Measurements");
    let (_directory, path, original, mut harness) = opened(before);
    let old = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &old);
    create_table(&mut harness);
    for (kind, name) in [
        ("table-draft-number", "Score"),
        ("table-draft-number", "ByteValue"),
        ("table-draft-number", "ShortValue"),
        ("table-draft-large-integer", "LargeValue"),
        ("table-draft-number", "SingleValue"),
        ("table-draft-number", "DoubleValue"),
        ("table-draft-date-time", "DateValue"),
    ] {
        add_named_field(&mut harness, kind, name);
    }
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(&mut harness, "ribbon-table-design");
    for (field, size) in [
        ("ByteValue", "table-draft-number-byte"),
        ("ShortValue", "table-draft-number-integer"),
        ("SingleValue", "table-draft-number-single"),
        ("DoubleValue", "table-draft-number-double"),
    ] {
        harness
            .get_by_role_and_label(Role::Button, field)
            .click_accesskit();
        harness.run_steps(2);
        choose(&mut harness, "design-field-size", size);
    }
    view(&mut harness, "ribbon-datasheet");
    for (index, (column, text)) in raw::COLUMNS_TO_EDIT.into_iter().zip(INPUT).enumerate() {
        reveal(&mut harness, column.name(), index >= 3);
        let label = field_label(&harness, "table-draft-record-field", column.name());
        assert!(
            harness
                .query_by_role_and_label(Role::TextInput, &label)
                .is_some(),
            "actual initial editor for {} is still clipped after scrolling",
            column.name()
        );
        replace(&mut harness, Role::TextInput, &label, text);
    }
    assert_image(&path, &original);
    save_schema_and_bind_record(&mut harness, "Measurements");
    assert_image(&path, &schema);
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_image(&path, &schema);
    apply_and_acknowledge(&mut harness);
    let mut expected = raw::append_values(&schema, definition, &INITIAL);
    assert_image(&path, &expected);
    // This ordinary LONG control must remain usable on the same typed table.
    reveal(&mut harness, "Score", false);
    begin_replacement(&mut harness, 0, "Score", false, Some("124"));
    let after = raw::update_score(&expected, definition, 0, Some(124));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    for (column, value, text) in REPLACEMENTS {
        for (was_null, supplied, input) in [
            (false, Some(value), Some(text)),
            (false, None, None),
            (true, Some(value), Some(text)),
        ] {
            reveal(&mut harness, column.name(), column.number() >= 5);
            begin_replacement(&mut harness, 0, column.name(), was_null, input);
            let after = raw::update(&expected, definition, 0, column, supplied);
            review_and_save(&mut harness, &path, &expected, &after);
            expected = after;
            assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
            assert!(!journal_path(&path).exists());
        }
    }
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    fresh
        .get_by_role_and_label(Role::Button, "Measurements")
        .focus();
    fresh.run_steps(1);
    fresh.key_press(egui::Key::Enter);
    fresh.run_steps(1);
    finish_reads(&mut fresh);
    let data = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Measurements");
    assert_eq!(data.columns.len(), 8);
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "124");
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

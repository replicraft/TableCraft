//! A complete native field declaration crosses real schema and row Save owners.
use super::test_support::{
    apply_and_acknowledge, field_label, initial_table, review, save_schema_and_bind_record,
};
use crate::workspace::{
    Desktop,
    test_support::{app_harness, finish_reads},
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_builders::table::DraftFieldType;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let mismatch = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && mismatch.is_none(),
        "complete image mismatch: lengths {} / {}, first byte {:?}",
        actual.len(),
        expected.len(),
        mismatch
    );
}

fn scroll_to_end(harness: &mut Harness<'_, Desktop>, last: bool) {
    harness.get_by_role_and_label(Role::Tab, "Home").focus();
    harness.run_steps(2);
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 400.0)));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(if last { -100_000.0 } else { 100_000.0 }, 0.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(10);
    let field = if last { "Field254" } else { "Field1" };
    assert!(
        harness
            .get_by_role_and_label(Role::ColumnHeader, field)
            .rect()
            .intersects(harness.ctx.content_rect())
    );
}

#[test]
fn all_255_fields_save_and_first_and_last_inputs_survive_both_reviewed_commits() {
    let source = raw::source();
    let (_directory, path, mut harness) = initial_table();
    // These are the same typed draft commands used by the gallery. The native
    // review and both consuming saves below still run through actual widgets.
    for _ in 1..255 {
        assert!(
            harness
                .state_mut()
                .state
                .add_table_draft_field(DraftFieldType::Number)
        );
    }
    assert!(
        !harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::Number)
    );
    harness.run_steps(2);
    // Commit the final default Field254 header through its real rename widget.
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    for (field, value) in [("Field1", "17"), ("Field254", "-29")] {
        scroll_to_end(&mut harness, field == "Field254");
        let label = field_label(&harness, "table-draft-record-field", field);
        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .type_text(value);
        harness.run_steps(1);
    }
    assert_image(&path, &source.before);
    save_schema_and_bind_record(&mut harness, "Table1");
    assert_image(&path, &source.after);
    assert!(!journal_path(&path).exists());
    for (field, value) in [("Field1", "17"), ("Field254", "-29")] {
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, field)
                .value()
                .as_deref(),
            Some(value)
        );
    }
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_image(&path, &source.after);
    apply_and_acknowledge(&mut harness);
    let expected = raw::rows::after_end_fields(
        &source.after,
        &source,
        Some(17),
        raw::rows::LastFieldInput::Value(-29),
    );
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.columns.len(), 255);
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "17");
    assert_eq!(data.rows[0][254].text, "-29");
    for cell in &data.rows[0][2..254] {
        assert_eq!(cell.text, "0");
    }
    assert_image(&path, &expected);
}

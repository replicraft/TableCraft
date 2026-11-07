use super::*;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

fn value(text: &str) -> InsertInput {
    InsertInput::Value(EditInput {
        text: text.into(),
        is_null: false,
    })
}

#[test]
fn declaration_sync_keeps_generated_omitted_and_preserves_field_identity_across_rename() {
    let mut table = TableDraft::initial();
    let field = table.add_field(DraftFieldType::ShortText).unwrap();
    let mut row = RowDraft::new();
    assert!(row.synchronize(&table));
    assert!(!row.is_modified());
    assert!(!row.set_input(table.fields()[0].id(), value("1")));
    assert!(row.set_input(field, value("Ada")));
    let revision = row.revision();
    table.rename_field(field, "Name").unwrap();
    assert!(row.synchronize(&table));
    assert_eq!(row.revision(), revision);
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap(),
        [InsertInput::Omitted, value("Ada")]
    );
    table.add_field(DraftFieldType::LongInteger).unwrap();
    assert!(row.synchronize(&table));
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap(),
        [InsertInput::Omitted, value("Ada"), InsertInput::Omitted]
    );
}

#[test]
fn omission_null_and_empty_value_remain_distinct_and_hidden_raw_text_is_retained() {
    let mut table = TableDraft::initial();
    let field = table.add_field(DraftFieldType::ShortText).unwrap();
    let mut row = RowDraft::new();
    row.synchronize(&table);
    assert!(row.set_input(field, value("Ada")));
    assert!(row.set_input(field, InsertInput::Omitted));
    assert!(row.is_modified());
    assert_eq!(row.entries[1].text, "Ada");
    assert_eq!(
        row.raw_texts(&table.snapshot("Items").unwrap()).unwrap(),
        ["", "Ada"]
    );
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap()[1],
        InsertInput::Omitted
    );
    let null = InsertInput::Value(EditInput {
        text: "Ada".into(),
        is_null: true,
    });
    assert!(row.set_input(field, null.clone()));
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap()[1],
        null
    );
    assert!(row.set_input(field, value("")));
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap()[1],
        value("")
    );
}

#[test]
fn oversized_input_revision_exhaustion_and_stale_schema_refuse_without_partial_changes() {
    let mut table = TableDraft::initial();
    let field = table.add_field(DraftFieldType::ShortText).unwrap();
    let mut row = RowDraft::new();
    row.synchronize(&table);
    row.set_input(field, value("before"));
    let revision = row.revision();
    assert!(!row.set_input(
        field,
        value(&"x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1))
    ));
    assert!(!row.set_input(field, value(&"x".repeat(MAX_RECORD_TEXT_BYTES + 1))));
    assert_eq!(row.revision(), revision + 1);
    assert!(row.has_rejected_input());
    assert!(row.inputs(&table.snapshot("Items").unwrap()).is_none());
    assert!(row.raw_texts(&table.snapshot("Items").unwrap()).is_none());
    assert_eq!(row.entries[1].text, "before");
    assert!(row.set_input(field, value("before")));
    assert!(!row.has_rejected_input());
    assert_eq!(row.revision(), revision + 2);
    let mut different = TableDraft::initial();
    different.add_field(DraftFieldType::LongInteger).unwrap();
    assert!(row.inputs(&different.snapshot("Items").unwrap()).is_none());
    assert!(
        row.raw_texts(&different.snapshot("Items").unwrap())
            .is_none()
    );
    row.revision = u64::MAX;
    assert!(!row.set_input(field, value("after")));
    assert_eq!(row.entries[1].text, "before");
}

#[test]
fn yes_no_has_no_fabricated_false_and_cannot_be_explicit_null() {
    let mut table = TableDraft::initial();
    let field = table.add_field(DraftFieldType::YesNo).unwrap();
    let mut row = RowDraft::new();
    row.synchronize(&table);
    assert!(!row.is_modified());
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap()[1],
        InsertInput::Omitted
    );
    assert!(!row.set_input(
        field,
        InsertInput::Value(EditInput {
            text: String::new(),
            is_null: true
        })
    ));
    assert!(!row.is_modified());
    assert!(row.set_input(field, value("false")));
}

#[test]
fn actual_cell_typing_preserves_raw_value_and_generated_cell_has_no_editor() {
    let mut table = TableDraft::initial();
    table.add_field(DraftFieldType::ShortText).unwrap();
    let mut row = RowDraft::new();
    row.synchronize(&table);
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (TableDraft, RowDraft)| {
            for field in state.0.fields() {
                state.1.show_cell(ui, field);
            }
        },
        (table, row),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let localizer = crate::Localizer::default();
    let label = |field: &str| {
        let mut arguments = crate::FluentArgs::new();
        arguments.set("field", field);
        localizer.format("table-draft-record-field", &arguments)
    };
    let generated_label = label("ID");
    let input_label = label("Field1");
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, &generated_label)
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::TextInput, &input_label)
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, &input_label)
        .type_text("Ada");
    harness.run_steps(2);
    let (table, row) = harness.state();
    assert_eq!(
        row.inputs(&table.snapshot("Items").unwrap()).unwrap(),
        [InsertInput::Omitted, value("Ada")]
    );
    assert!(row.is_modified());
    let revision = row.revision();
    harness.event(egui::Event::Paste(
        "x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1),
    ));
    harness.run_steps(2);
    assert!(harness.state().1.has_rejected_input());
    assert_eq!(harness.state().1.revision(), revision + 1);
    assert_eq!(harness.state().1.entries[1].text, "Ada");
    harness
        .get_by_role_and_label(Role::TextInput, &input_label)
        .type_text("x");
    harness.run_steps(2);
    assert!(!harness.state().1.has_rejected_input());
    assert_eq!(harness.state().1.entries[1].text, "Adax");
}

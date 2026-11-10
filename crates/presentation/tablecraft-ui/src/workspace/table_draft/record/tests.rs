use super::*;
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

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
    table.add_field(DraftFieldType::Number).unwrap();
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
    different.add_field(DraftFieldType::Number).unwrap();
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

#[test]
fn boolean_checkbox_keeps_omission_mixed_and_writes_only_explicit_canonical_values() {
    for dark in [false, true] {
        let mut table = TableDraft::initial();
        table.add_field(DraftFieldType::YesNo).unwrap();
        let mut row = RowDraft::new();
        row.synchronize(&table);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(220.0, 120.0))
            .build_ui_state(
                |ui, state: &mut (TableDraft, RowDraft)| {
                    ui.allocate_ui(egui::vec2(180.0, 24.0), |ui| {
                        state.1.show_cell(ui, &state.0.fields()[1]);
                    });
                },
                (table, row),
            );
        crate::configure(&harness.ctx);
        harness.ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        harness.run_steps(2);
        let localizer = crate::Localizer::default();
        let mut args = crate::FluentArgs::new();
        args.set("field", "Field1");
        let label = localizer.format("table-draft-record-field", &args);
        let mode_label = localizer.format("insertion-field-input", &args);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::CheckBox, &label)
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::Mixed)
        );
        assert!(!harness.state().1.is_modified());
        harness
            .get_by_role_and_label(Role::CheckBox, &label)
            .click_accesskit();
        harness.run_steps(2);
        let snapshot = harness.state().0.snapshot("Items").unwrap();
        assert_eq!(
            harness.state().1.inputs(&snapshot).unwrap()[1],
            value("true")
        );
        harness
            .get_by_role_and_label(Role::CheckBox, &label)
            .focus();
        harness.run_steps(1);
        harness.key_press(egui::Key::Space);
        harness.run_steps(2);
        assert_eq!(
            harness.state().1.inputs(&snapshot).unwrap()[1],
            value("false")
        );
        harness
            .get_by_role_and_label(Role::Button, &mode_label)
            .click_accesskit();
        harness.run_steps(2);
        assert!(
            harness
                .get_by_role_and_label(Role::RadioButton, "NULL")
                .accesskit_node()
                .is_disabled()
        );
        harness
            .get_by_role_and_label(Role::RadioButton, "Omitted")
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(
            harness.state().1.inputs(&snapshot).unwrap()[1],
            InsertInput::Omitted
        );
        assert_eq!(harness.state().1.raw_texts(&snapshot).unwrap()[1], "false");
        assert_eq!(
            harness
                .get_by_role_and_label(Role::CheckBox, &label)
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::Mixed)
        );
        harness
            .get_by_role_and_label(Role::Button, &mode_label)
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::RadioButton, "Value")
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(
            harness.state().1.inputs(&snapshot).unwrap()[1],
            value("false")
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::CheckBox, &label)
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::False)
        );
    }
}

#[test]
fn boolean_checkbox_is_centered_on_the_full_field_with_a_separate_mode_gutter() {
    let mut table = TableDraft::initial();
    table.add_field(DraftFieldType::YesNo).unwrap();
    let mut row = RowDraft::new();
    row.synchronize(&table);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(240.0, 100.0))
        .build_ui_state(
            |ui, state: &mut (TableDraft, RowDraft, Option<egui::Rect>)| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(150.0, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        state.2 = Some(ui.max_rect());
                        state.1.show_cell(ui, &state.0.fields()[1]);
                    },
                );
            },
            (table, row, None),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let mut arguments = crate::FluentArgs::new();
    arguments.set("field", "Field1");
    let localizer = crate::Localizer::default();
    let label = localizer.format("table-draft-record-field", &arguments);
    let mode_label = localizer.format("insertion-field-input", &arguments);
    let field = harness.state().2.unwrap();
    let checkbox = harness.get_by_role_and_label(Role::CheckBox, &label).rect();
    let mode = harness
        .get_by_role_and_label(Role::Button, &mode_label)
        .rect();
    assert!((checkbox.center().x - field.center().x).abs() < 0.5);
    assert!((checkbox.center().y - field.center().y).abs() < 0.5);
    assert!(checkbox.right() < mode.left());
    assert!(field.contains_rect(checkbox));
}

#[test]
fn noncanonical_boolean_text_stays_editable_until_explicitly_corrected() {
    for raw in ["True", "FALSE", "maybe"] {
        let mut table = TableDraft::initial();
        let field = table.add_field(DraftFieldType::YesNo).unwrap();
        let mut row = RowDraft::new();
        assert!(row.synchronize(&table));
        assert!(row.set_input(field, value(raw)));
        let mut harness = Harness::builder().build_ui_state(
            |ui, state: &mut (TableDraft, RowDraft)| {
                state.1.show_cell(ui, &state.0.fields()[1]);
            },
            (table, row),
        );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        let mut arguments = crate::FluentArgs::new();
        arguments.set("field", "Field1");
        let label = crate::Localizer::default().format("table-draft-record-field", &arguments);
        assert!(
            harness
                .query_by_role_and_label(Role::CheckBox, &label)
                .is_none()
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, &label)
                .value()
                .as_deref(),
            Some(raw)
        );
        let snapshot = harness.state().0.snapshot("Items").unwrap();
        assert_eq!(harness.state().1.inputs(&snapshot).unwrap()[1], value(raw));

        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .focus();
        harness.run_steps(1);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.event(egui::Event::Text("true".into()));
        harness.run_steps(2);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::CheckBox, &label)
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::True)
        );
        assert_eq!(
            harness.state().1.inputs(&snapshot).unwrap()[1],
            value("true")
        );
    }
}

#[test]
fn field_removal_preflight_keeps_input_and_refuses_exhausted_revision() {
    let mut table = TableDraft::initial();
    let removed = table.add_field(DraftFieldType::ShortText).unwrap();
    let kept = table.add_field(DraftFieldType::Number).unwrap();
    let mut row = RowDraft::new();
    assert!(row.synchronize(&table));
    assert!(row.set_input(removed, value("hidden retained text")));
    assert!(row.set_input(removed, InsertInput::Omitted));
    assert!(row.set_input(kept, value("0007")));
    let snapshot = table.snapshot("Items").unwrap();
    let original = row.raw_texts(&snapshot).unwrap();
    assert!(row.removal(table.fields()[0].id()).is_none());
    row.revision = u64::MAX;
    assert!(row.removal(removed).is_none());
    assert_eq!(row.raw_texts(&snapshot).unwrap(), original);
    assert_eq!(row.revision(), u64::MAX);
}

#[test]
fn declaration_type_changes_keep_raw_modes_and_allocation_instead_of_resetting_input() {
    for mode in [Choice::Omitted, Choice::Value, Choice::Null] {
        let mut table = TableDraft::initial();
        let field = table.add_field(DraftFieldType::ShortText).unwrap();
        let mut row = RowDraft::new();
        assert!(row.synchronize(&table));
        assert!(row.set_input(
            field,
            InsertInput::Value(EditInput {
                text: "0007 Ω original".into(),
                is_null: mode == Choice::Null,
            })
        ));
        if mode == Choice::Omitted {
            assert!(row.set_input(field, InsertInput::Omitted));
        }
        let allocation = row.entries[1].text.as_ptr();
        for kind in [
            DraftFieldType::Number,
            DraftFieldType::YesNo,
            DraftFieldType::Currency,
        ] {
            table.set_field_type(field, kind).unwrap();
            assert!(row.synchronize(&table));
            assert!(row.entries[1].choice == mode);
            assert_eq!(row.entries[1].text, "0007 Ω original");
            assert_eq!(row.entries[1].text.as_ptr(), allocation);
            assert_eq!(row.entries[1].kind, kind);
        }
        row.revision = u64::MAX;
        assert!(row.type_change(field).is_none());
        assert_eq!(row.entries[1].text.as_ptr(), allocation);
        assert_eq!(row.entries[1].text, "0007 Ω original");
    }
}

#[test]
fn generated_primary_preflight_preserves_raw_allocations_and_refuses_exhausted_revision() {
    let mut draft = TableDraft::design();
    let field = draft.add_field(DraftFieldType::ShortText).unwrap();
    let mut record = RowDraft::new();
    assert!(record.synchronize(&draft));
    assert!(record.set_input(field, value("  exact 日本語  ")));
    let pointer = record.entries[0].text.as_ptr();
    let original = record.revision;
    record.revision = u64::MAX;
    assert!(record.prepare_generated_primary(&draft).is_none());
    assert_eq!(record.entries.len(), 1);
    assert_eq!(record.entries[0].text.as_ptr(), pointer);
    record.revision = original;
    let revision = record.prepare_generated_primary(&draft).unwrap();
    let generated = draft.add_incrementing_primary_key().unwrap();
    record.prepend_generated_primary(generated, revision);
    assert_eq!(record.entries[1].text.as_ptr(), pointer);
    let snapshot = draft.snapshot("Items").unwrap();
    assert_eq!(
        record.inputs(&snapshot).unwrap(),
        vec![InsertInput::Omitted, value("  exact 日本語  ")]
    );
    assert_eq!(
        record.raw_texts(&snapshot).unwrap(),
        vec![String::new(), "  exact 日本語  ".into()]
    );
}

#[test]
fn exhausted_record_revision_refuses_typed_and_datasheet_additions_without_schema_changes() {
    let mut preview = crate::preview::Preview::new(crate::preview::Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    // No render occurred: the shared append helper must first initialize the
    // existing ID cell before reserving the ordinary field's row entry.
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.commit_rename());
    let field = editor.draft.fields()[1].id();
    assert!(editor.record.set_input(field, value("  retained 日本語  ")));
    editor.record.revision = u64::MAX;
    let before = editor.draft.snapshot("Table1").unwrap();
    let inputs = editor.record.inputs(&before).unwrap();
    let raw_pointer = editor.record.entries[1].text.as_ptr();
    assert!(!state.add_table_draft_field(DraftFieldType::Number));
    let editor = state.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.inputs(&before).unwrap(), inputs);
    assert_eq!(editor.record.entries[1].text.as_ptr(), raw_pointer);
    assert_eq!(editor.record.revision, u64::MAX);
    assert!(editor.error);
    assert!(editor.rename.is_none());

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (crate::State, Vec<crate::Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.inputs(&before).unwrap(), inputs);
    assert_eq!(editor.record.entries[1].text.as_ptr(), raw_pointer);
    assert_eq!(editor.record.revision, u64::MAX);
    assert!(editor.error);
    assert!(editor.rename.is_none());
    assert!(harness.state().1.is_empty());
}

#[test]
fn update_restart_keeps_blank_and_typed_initial_record_drafts_intact() {
    let mut state = crate::workspace::test_support::saved_table("Existing database", Vec::new());
    assert!(state.can_restart_for_update());
    assert!(state.open_initial_table_draft());
    assert!(!state.has_modified_table_drafts());
    assert!(!state.can_restart_for_update());

    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.commit_rename());
    let field = editor.draft.fields()[1].id();
    assert!(editor.record.set_input(field, value("  retained 日本語  ")));
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let original = editor.record.inputs(&snapshot).unwrap();
    let pointer = editor.record.entries[1].text.as_ptr();
    assert!(!state.can_restart_for_update());
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.record.inputs(&snapshot).unwrap(), original);
    assert_eq!(editor.record.entries[1].text.as_ptr(), pointer);
}

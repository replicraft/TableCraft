use super::*;
use crate::workspace::insertion::{
    inputs as current_inputs,
    tests::{choose_mode, state_with_fields},
};
use crate::{Action, EditInput, InsertField};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::TableDraft;

fn fixture() -> (State, InitialRecord) {
    let mut draft = TableDraft::initial();
    for kind in [
        DraftFieldType::ShortText,
        DraftFieldType::Number,
        DraftFieldType::LargeInteger,
        DraftFieldType::YesNo,
        DraftFieldType::Currency,
        DraftFieldType::DateTime,
    ] {
        draft.add_field(kind).unwrap();
    }
    let snapshot = draft.snapshot("Empty table").unwrap();
    let hints = [
        InputHint::Int32,
        InputHint::Text,
        InputHint::Int32,
        InputHint::Int64,
        InputHint::Boolean,
        InputHint::Currency,
        InputHint::DateTimeIso,
    ];
    let fields = snapshot
        .fields()
        .iter()
        .zip(hints)
        .enumerate()
        .map(|(position, (field, input_hint))| InsertField {
            name: field.name().into(),
            input_hint,
            nullable: true,
            auto_number: position == 0,
        })
        .collect();
    let (state, _) = state_with_fields(fields);
    let inputs = vec![
        InsertInput::Omitted,
        value("  original text  ", false),
        InsertInput::Omitted,
        value("hidden null text", true),
        value("True", false),
        value("12.3456", false),
        value("2008-09-18", false),
    ];
    let raw_text = inputs
        .iter()
        .map(|input| match input {
            InsertInput::Omitted => String::new(),
            InsertInput::Value(value) => value.text.clone(),
        })
        .collect();
    (
        state,
        InitialRecord {
            snapshot,
            inputs,
            raw_text,
        },
    )
}

fn value(text: &str, is_null: bool) -> InsertInput {
    InsertInput::Value(EditInput {
        text: text.into(),
        is_null,
    })
}

fn entries(state: &State) -> Vec<InsertInput> {
    let Some(Dialog::Draft { entries, .. }) = &state.insertion else {
        panic!("expected retained insertion entries");
    };
    current_inputs(entries)
}

#[test]
fn initial_seed_preserves_exact_presence_and_raw_text_without_creating_review_or_rows() {
    let (mut state, record) = fixture();
    assert!(state.seed_initial_insert(20, &record));
    assert_eq!(entries(&state), record.inputs);
    assert!(state.seed_initial_insert(20, &record));
    assert!(matches!(
        &state.insertion,
        Some(Dialog::Draft {
            pending: None,
            review: None,
            ..
        })
    ));
    assert!(state.data.as_ref().unwrap().rows.is_empty());
    let mut changed = record;
    changed.inputs[1] = value("replacement", false);
    changed.raw_text[1] = "replacement".into();
    assert!(!state.seed_initial_insert(20, &changed));
    assert_ne!(entries(&state), changed.inputs);
}

#[test]
fn stale_mismatched_generated_oversized_and_pending_seeds_leave_all_entries_unchanged() {
    for case in 0..14 {
        let (mut state, mut record) = fixture();
        let mut id = 20;
        if let Some(Dialog::Draft { draft, pending, .. }) = &mut state.insertion {
            match case {
                0 => id = 21,
                1 => draft.target.table = "Other table".into(),
                2 => draft.fields[1].name = "OtherField".into(),
                3 => draft.fields[1].input_hint = InputHint::BinaryHex,
                4 => draft.fields[1].auto_number = true,
                5 => record.inputs[0] = value("1", false),
                6 => {
                    record.inputs.pop();
                }
                7 => {
                    record.inputs[1] = value(
                        &"x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1),
                        false,
                    )
                }
                8 => draft.target.document_revision += 1,
                9 => *pending = Some(record.inputs.clone()),
                10 => {
                    record.raw_text.pop();
                }
                11 => record.raw_text[2] = "x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1),
                12 => record.raw_text[1] = "different supplied text".into(),
                13 => record.raw_text[0] = "hidden generated text".into(),
                _ => unreachable!(),
            }
        }
        let before = entries(&state);
        assert!(!state.seed_initial_insert(id, &record), "case {case}");
        assert_eq!(entries(&state), before, "case {case}");
        assert!(state.data.as_ref().unwrap().rows.is_empty());
    }
}

#[test]
fn an_existing_user_entry_is_never_overwritten_by_the_initial_handoff() {
    let (mut state, record) = fixture();
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[1].choice = Choice::Value;
        entries[1].text = "user corrected this".into();
    }
    let before = entries(&state);
    assert!(!state.seed_initial_insert(20, &record));
    assert_eq!(entries(&state), before);
}

#[test]
fn seeded_values_render_in_the_actual_inline_row_without_dispatch_or_fabricated_records() {
    let (mut state, record) = fixture();
    assert!(state.seed_initial_insert(20, &record));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 650.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let field = harness.get_by_role_and_label(Role::TextInput, "Field1");
    assert_eq!(
        field.accesskit_node().value().as_deref(),
        Some("  original text  ")
    );
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(entries(&harness.state().0), record.inputs);
    assert!(!harness.state_mut().0.seed_initial_insert(20, &record));
}

#[test]
fn omitted_hidden_text_survives_schema_handoff_and_returns_when_value_mode_is_selected() {
    let (mut state, mut record) = fixture();
    record.raw_text[2] = "  00042 retained  ".into();
    assert!(state.seed_initial_insert(20, &record));
    assert!(state.seed_initial_insert(20, &record));
    assert_eq!(entries(&state)[2], InsertInput::Omitted);
    let Some(Dialog::Draft {
        entries: retained, ..
    }) = &state.insertion
    else {
        panic!("expected seeded inline entries");
    };
    assert_eq!(retained[2].text, "  00042 retained  ");
    record.raw_text[2] = "overwrite attempt".into();
    assert!(!state.seed_initial_insert(20, &record));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 650.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    choose_mode(&mut harness, "Field2", "Value");
    assert_eq!(
        entries(&harness.state().0)[2],
        value("  00042 retained  ", false)
    );
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn number_size_handoff_requires_its_exact_native_input_hint_without_changing_raw_input() {
    for (size, expected) in [
        (DraftNumberSize::Byte, InputHint::Byte),
        (DraftNumberSize::Integer, InputHint::Int16),
        (DraftNumberSize::LongInteger, InputHint::Int32),
        (DraftNumberSize::Single, InputHint::Single),
        (DraftNumberSize::Double, InputHint::Double),
        (DraftNumberSize::Decimal, InputHint::Decimal),
        (DraftNumberSize::ReplicationId, InputHint::Guid),
    ] {
        let (mut state, mut record) = fixture();
        let mut draft = TableDraft::initial();
        for source in &record.snapshot.fields()[1..] {
            draft.add_field(source.kind()).unwrap();
        }
        let field = draft.fields()[2].id();
        draft.set_field_number_size(field, size).unwrap();
        record.snapshot = draft.snapshot("Empty table").unwrap();
        record.raw_text[2] = "0007 raw unchanged".into();
        if let Some(Dialog::Draft { draft, .. }) = &mut state.insertion {
            draft.fields[2].input_hint = InputHint::BinaryHex;
        }
        assert!(!state.seed_initial_insert(20, &record));
        if let Some(Dialog::Draft { draft, .. }) = &mut state.insertion {
            draft.fields[2].input_hint = expected;
        }
        assert!(state.seed_initial_insert(20, &record));
        assert!(matches!(entries(&state)[2], InsertInput::Omitted));
        let Some(Dialog::Draft { entries, .. }) = &state.insertion else {
            panic!("expected retained number input");
        };
        assert_eq!(entries[2].text, "0007 raw unchanged");
    }
}

fn manual_fixture(primary: bool) -> (State, InitialRecord) {
    let mut declaration = TableDraft::design();
    declaration.add_field(DraftFieldType::ShortText).unwrap();
    let key = declaration.add_field(DraftFieldType::Number).unwrap();
    if primary {
        declaration.set_primary_key(Some(key)).unwrap();
    }
    let snapshot = declaration.snapshot("Empty table").unwrap();
    let fields = snapshot
        .fields()
        .iter()
        .zip([InputHint::Text, InputHint::Int32])
        .map(|(field, input_hint)| InsertField {
            name: field.name().into(),
            input_hint,
            nullable: true,
            auto_number: false,
        })
        .collect();
    let (state, _) = state_with_fields(fields);
    (
        state,
        InitialRecord {
            snapshot,
            inputs: vec![value("  Ada 日本語  ", false), value("0007", false)],
            raw_text: vec!["  Ada 日本語  ".into(), "0007".into()],
        },
    )
}

#[test]
fn manual_keyless_and_nonfirst_primary_continue_exact_original_inputs_without_generation() {
    for primary in [false, true] {
        let (mut state, record) = manual_fixture(primary);
        assert_eq!(record.snapshot.fields()[1].is_primary(), primary);
        assert!(!record.snapshot.fields()[1].is_protected());
        assert!(state.seed_initial_insert(20, &record));
        assert_eq!(entries(&state), record.inputs);
        assert!(state.seed_initial_insert(20, &record));
        assert_eq!(entries(&state), record.inputs);
        assert!(matches!(
            &state.insertion,
            Some(Dialog::Draft {
                pending: None,
                review: None,
                ..
            })
        ));
        assert!(state.data.as_ref().unwrap().rows.is_empty());
    }
}

#[test]
fn manual_primary_continuation_refuses_wrong_native_generation_or_type_without_input_effects() {
    for primary in [false, true] {
        for wrong in 0..3 {
            let (mut state, record) = manual_fixture(primary);
            let Some(Dialog::Draft { draft, .. }) = &mut state.insertion else {
                panic!("native draft");
            };
            match wrong {
                0 => draft.fields[0].auto_number = true,
                1 => draft.fields[1].auto_number = true,
                _ => draft.fields[1].input_hint = InputHint::Int64,
            }
            let before = entries(&state);
            assert!(!state.seed_initial_insert(20, &record));
            assert_eq!(entries(&state), before);
            assert!(state.data.as_ref().unwrap().rows.is_empty());
        }
    }
}

use super::*;
use crate::workspace::insertion::{
    Dialog,
    tests::{retained_entry, state_with_fields},
};
use crate::{EditInput, InputHint, InsertDraft, InsertField};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{DraftFieldType, TableDraft};

fn prepared() -> (State, InsertTarget, Vec<InsertField>) {
    let mut declaration = TableDraft::initial();
    declaration.add_field(DraftFieldType::ShortText).unwrap();
    declaration.add_field(DraftFieldType::LongInteger).unwrap();
    let snapshot = declaration.snapshot("Empty table").unwrap();
    let fields = [InputHint::Int32, InputHint::Text, InputHint::Int32]
        .into_iter()
        .zip(snapshot.fields())
        .enumerate()
        .map(|(position, (input_hint, original))| InsertField {
            name: original.name().into(),
            input_hint,
            nullable: true,
            auto_number: position == 0,
        })
        .collect();
    let (mut state, target) = state_with_fields(fields);
    let Some(Dialog::Draft { draft, .. }) = state.insertion.take() else {
        panic!("expected the shared native insertion fixture");
    };
    let record = InitialRecord {
        snapshot,
        inputs: vec![
            InsertInput::Omitted,
            InsertInput::Value(EditInput {
                text: "  retained name  ".into(),
                is_null: false,
            }),
            InsertInput::Omitted,
        ],
        raw_text: vec![
            String::new(),
            "  retained name  ".into(),
            " 00019 hidden ".into(),
        ],
    };
    state.insert_continuation = Some(
        state
            .prepare_initial_record(
                &ObjectTarget {
                    document_revision: target.document_revision,
                    kind: ObjectKind::Table {
                        name: target.table.clone(),
                    },
                },
                record,
            )
            .ok()
            .expect("prepare the exact saved table continuation"),
    );
    (state, target, draft.fields)
}

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 600.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| {
                state.0.insert_continuation_dialog(ui.ctx(), &mut state.1)
            },
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
}

fn assert_retained(state: &State) {
    let retained = &state
        .insert_continuation
        .as_ref()
        .expect("retain the original record")
        .source;
    let Seed::Initial(retained) = retained else {
        panic!("initial schema record retained");
    };
    assert_eq!(retained.raw_text[1], "  retained name  ");
    assert_eq!(retained.raw_text[2], " 00019 hidden ");
    assert_eq!(retained.inputs[2], InsertInput::Omitted);
    assert!(state.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn prepared_record_emits_one_begin_then_transfers_only_into_the_actual_current_draft() {
    let (state, target, fields) = prepared();
    let mut harness = harness(state);
    assert_eq!(harness.state().1.len(), 1);
    assert!(
        matches!(&harness.state().1[0], Action::BeginInsert { target: actual } if actual == &target)
    );
    harness.run_steps(3);
    assert_eq!(harness.state().1.len(), 1);
    let state = &mut harness.state_mut().0;
    state.begin_insert(target.clone(), 71);
    state.set_insert_draft(
        71,
        InsertDraft {
            id: 99,
            target: target.clone(),
            fields,
        },
    );
    assert!(!state.insert_continuation_pending());
    assert_eq!(state.initial_record_notice, Some(target));
    let Some(Dialog::Draft {
        draft,
        pending,
        review,
        ..
    }) = &state.insertion
    else {
        panic!("expected the actual native draft to own the record");
    };
    assert_eq!(draft.id, 99);
    assert!(pending.is_none() && review.is_none());
    assert_eq!(
        retained_entry(state, 2),
        (InsertInput::Omitted, " 00019 hidden ")
    );
    assert!(state.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn mismatched_or_stale_current_response_keeps_raw_input_and_offers_explicit_retry_or_discard() {
    for stale in [false, true] {
        let (mut state, target, mut fields) = prepared();
        state.begin_insert(target.clone(), 71);
        let mut returned = target;
        if stale {
            returned.document_revision += 1;
        } else {
            fields[1].name = "DifferentField".into();
        }
        state.set_insert_draft(
            71,
            InsertDraft {
                id: 99,
                target: returned,
                fields,
            },
        );
        assert_retained(&state);
        assert!(state.insertion.is_none());
        let harness = harness(state);
        assert!(harness.state().1.is_empty());
        harness.get_by_role_and_label(Role::Button, "Retry new record");
        harness.get_by_role_and_label(Role::Button, "Discard new record");
        harness.get_by_label("The table was saved; this record has not been saved.");
        assert_retained(&harness.state().0);
    }
}

#[test]
fn begin_failure_and_close_keep_the_record_until_an_explicit_retry() {
    let (mut state, target, _) = prepared();
    state.begin_insert(target.clone(), 71);
    state.fail_insert(71, "native admission refused".into());
    assert!(state.close_insert());
    assert_retained(&state);
    let mut harness = harness(state);
    assert!(harness.state().1.is_empty());
    harness.run_steps(3);
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Retry new record")
        .click_accesskit();
    harness.run_steps(3);
    assert_eq!(harness.state().1.len(), 1);
    assert!(
        matches!(&harness.state().1[0], Action::BeginInsert { target: actual } if actual == &target)
    );
    assert_retained(&harness.state().0);
}

#[test]
fn stale_database_disables_retry_without_loading_or_starting_another_source() {
    let (mut state, _, _) = prepared();
    state.insert_continuation.as_mut().unwrap().auto_start = false;
    state.database.as_mut().unwrap().document_revision += 1;
    let mut harness = harness(state);
    let retry = harness.get_by_role_and_label(Role::Button, "Retry new record");
    assert!(retry.accesskit_node().is_disabled());
    harness.run_steps(4);
    assert!(harness.state().1.is_empty());
    assert_retained(&harness.state().0);
    harness
        .get_by_role_and_label(Role::Button, "Discard new record")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().0.insert_continuation_pending());
    assert!(
        harness
            .state()
            .0
            .database
            .as_ref()
            .unwrap()
            .tables
            .iter()
            .any(|table| table.name == "Empty table")
    );
    assert!(harness.state().1.is_empty());
}

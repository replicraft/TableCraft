use super::*;
use crate::{Action, Data, Database, Table};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn state() -> (State, InsertTarget) {
    state_with_fields(vec![InsertField {
        name: "Name".into(),
        input_hint: crate::InputHint::Text,
        nullable: true,
        auto_number: false,
    }])
}

fn state_with_fields(fields: Vec<InsertField>) -> (State, InsertTarget) {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Insertion".into(),
        format: "ACE 16".into(),
        tables: vec![Table {
            local: true,
            name: "Empty table".into(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    });
    state.set_table_data(Data {
        row_ordinals: None,
        source_revision: Some(7),
        table: "Empty table".into(),
        offset: 0,
        columns: fields
            .iter()
            .map(|field| field.name.clone().into())
            .collect(),
        rows: Vec::new(),
        has_more: false,
    });
    let target = InsertTarget {
        document_revision: 7,
        table: "Empty table".into(),
    };
    assert!(
        state.can_insert_row(&target),
        "new rows do not require a fabricated existing row handle"
    );
    state.begin_insert(target.clone(), 1);
    state.set_insert_draft(
        1,
        InsertDraft {
            id: 20,
            target: target.clone(),
            fields,
        },
    );
    (state, target)
}

#[test]
fn explicit_omitted_null_and_value_choices_preserve_text_and_exact_review_intent() {
    let (state, _) = state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    harness
        .get_by_role_and_label(Role::RadioButton, "Value")
        .click_accesskit();
    harness.run();
    let value = harness.get_by_role(Role::MultilineTextInput);
    assert!(
        value.is_focused(),
        "choosing Value puts the caret in that field"
    );
    assert!(
        value.rect().width() > 500.0,
        "the input uses the bounded dialog width"
    );
    let cancel = harness.get_by_role_and_label(Role::Button, "Cancel");
    let review = harness.get_by_role_and_label(Role::Button, "Review new row");
    assert!(review.rect().left() < cancel.rect().left());
    assert!(
        (cancel.rect().right() - value.rect().right()).abs() <= 1.0,
        "the unchanged action order is aligned to the dialog's right edge"
    );
    harness
        .get_by_role(Role::MultilineTextInput)
        .type_text("Ada");
    harness.run();
    harness
        .get_by_role_and_label(Role::RadioButton, "Omitted")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ReviewInsert { draft_id: 20, inputs }] if inputs == &[InsertInput::Omitted])
    );
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(Role::RadioButton, "Value")
        .click_accesskit();
    harness.run();
    assert_eq!(
        harness
            .get_by_role(Role::MultilineTextInput)
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Ada")
    );
    harness
        .get_by_role_and_label(Role::RadioButton, "NULL")
        .click_accesskit();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ReviewInsert { draft_id: 20, inputs }] if inputs == &[InsertInput::Value(EditInput { text: "Ada".into(), is_null: true })])
    );
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn review_is_bound_to_current_intent_and_retained_save_owner_cannot_be_closed() {
    let (mut state, target) = state();
    let omitted = vec![InsertInput::Omitted];
    state.begin_insert_review(20, omitted.clone(), 2);
    let review = InsertReview {
        id: 30,
        draft_id: 20,
        target,
        inputs: omitted,
        inserted_rows: 1,
    };
    assert!(!state.set_insert_review(
        1,
        InsertReview {
            id: 29,
            draft_id: 20,
            target: review.target.clone(),
            inputs: review.inputs.clone(),
            inserted_rows: 1,
        }
    ));
    assert!(state.set_insert_review(2, review));
    assert!(state.can_apply_insert(30));
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0].choice = Choice::Null;
    }
    assert!(!state.can_apply_insert(30));
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0].choice = Choice::Omitted;
    }
    state.begin_insert_save(30, 40);
    assert!(state.insert_save_pending());
    assert!(!state.close_insert());
    state.set_insert_save_outcome(
        40,
        EditSaveOutcome::Retained {
            message: "Diagnosis retained; current operation outcome is unknown.".into(),
            can_recover: false,
            can_release: false,
        },
    );
    assert!(!state.close_insert());
    state.invalidate_insert();
    assert!(state.insert_save_pending());
    state.begin_insert_resolution(40, 41);
    assert!(state.insert_save_pending());
    state.set_insert_save_outcome(40, EditSaveOutcome::Saved("Stale receipt".into()));
    assert!(matches!(
        state.insertion,
        Some(Dialog::Saving { save_id: 41, .. })
    ));
    state.set_insert_save_outcome(
        41,
        EditSaveOutcome::Failed("Resources released; prior commit receipt remains known.".into()),
    );
    assert!(state.close_insert());
}

fn generated_state() -> (State, InsertTarget) {
    state_with_fields(vec![
        InsertField {
            name: "Sequence".into(),
            input_hint: crate::InputHint::Int32,
            nullable: false,
            auto_number: true,
        },
        InsertField {
            name: "ID".into(),
            input_hint: crate::InputHint::Int32,
            nullable: true,
            auto_number: false,
        },
    ])
}

#[test]
fn actual_auto_number_metadata_is_read_only_while_an_ordinary_id_stays_editable() {
    let (state, _) = generated_state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    let generated = harness.get_by_role_and_label(Role::Label, "(New)");
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::SetValue)
    );
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus)
    );
    assert_eq!(harness.get_all_by_role(Role::MultilineTextInput).count(), 1);
    for choice in ["Omitted", "NULL", "Value"] {
        assert_eq!(
            harness
                .get_all_by_role_and_label(Role::RadioButton, choice)
                .count(),
            1
        );
    }
    harness
        .get_by_role_and_label(Role::RadioButton, "Value")
        .click_accesskit();
    harness.run();
    harness.get_by_role(Role::MultilineTextInput).type_text("9");
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ReviewInsert { draft_id: 20, inputs }]
        if inputs == &[InsertInput::Omitted, InsertInput::Value(EditInput { text: "9".into(), is_null: false })])
    );
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn generated_value_or_null_cannot_be_admitted_even_if_private_entry_state_is_changed() {
    let (mut state, target) = generated_state();
    for choice in [Choice::Value, Choice::Null] {
        if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
            entries[0].choice = choice;
            entries[0].text = "123".into();
        }
        let proposed = match &state.insertion {
            Some(Dialog::Draft { entries, .. }) => inputs(entries),
            _ => unreachable!(),
        };
        assert!(!state.can_review_insert(20, &proposed));
        state.begin_insert_review(20, proposed, 2);
        assert!(matches!(
            &state.insertion,
            Some(Dialog::Draft { pending: None, .. })
        ));
    }
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0] = FieldInput::default();
    }
    let omitted = vec![InsertInput::Omitted, InsertInput::Omitted];
    assert!(state.can_review_insert(20, &omitted));
    state.begin_insert_review(20, omitted.clone(), 3);
    assert!(state.set_insert_review(
        3,
        InsertReview {
            id: 30,
            draft_id: 20,
            target,
            inputs: omitted,
            inserted_rows: 1,
        }
    ));
    assert!(state.can_apply_insert(30));
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0].choice = Choice::Value;
        entries[0].text = "123".into();
    }
    assert!(!state.can_apply_insert(30));
}

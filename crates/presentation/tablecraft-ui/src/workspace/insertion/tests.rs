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

pub(in crate::workspace) fn state_with_fields(fields: Vec<InsertField>) -> (State, InsertTarget) {
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

pub(in crate::workspace) fn retained_entry(state: &State, position: usize) -> (InsertInput, &str) {
    let Some(Dialog::Draft { entries, .. }) = &state.insertion else {
        panic!("expected retained insertion input");
    };
    (entries[position].input(), &entries[position].text)
}

pub(super) fn choose_mode(
    harness: &mut Harness<'_, (State, Vec<Action>)>,
    field: &str,
    mode: &str,
) {
    let mut arguments = crate::FluentArgs::new();
    arguments.set("field", field.to_owned());
    let label = harness
        .state()
        .0
        .localizer
        .format("insertion-field-input", &arguments);
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::RadioButton, mode)
        .click_accesskit();
    harness.run_steps(2);
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
    choose_mode(&mut harness, "Name", "Value");
    let value = harness.get_by_role_and_label(Role::TextInput, "Name");
    assert!(value.is_focused());
    assert!(value.rect().width() > 40.0);
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .type_text("Ada");
    harness.run();
    choose_mode(&mut harness, "Name", "Omitted");
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run();
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ReviewInsert { draft_id: 20, inputs }] if inputs == &[InsertInput::Omitted])
    );
    harness.state_mut().1.clear();
    choose_mode(&mut harness, "Name", "Value");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Ada")
    );
    choose_mode(&mut harness, "Name", "NULL");
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
    assert!(!state.close_insert());
    assert!(state.acknowledge_insert_save(41, InsertRetryPermission::Blocked));
    assert!(state.insert_continuation_pending());
    assert!(state.discard_retained_insert(41));
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
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Sequence")
            .is_none()
    );
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::TextInput, "ID")
            .count(),
        1
    );
    choose_mode(&mut harness, "ID", "Value");
    harness
        .get_by_role_and_label(Role::TextInput, "ID")
        .type_text("9");
    harness.run_steps(2);
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

#[test]
fn inline_escape_restores_field_checkpoint_before_requesting_record_cancel() {
    let (state, _) = state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .type_text("Keep exact input");
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().1.is_empty());
    assert!(
        matches!(&harness.state().0.insertion, Some(Dialog::Draft { entries, .. }) if entries[0].choice == Choice::Omitted && entries[0].text.is_empty())
    );
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(matches!(
        harness.state().1.as_slice(),
        [Action::CloseInsert {
            draft_id: Some(20),
            ..
        }]
    ));
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn inline_review_failure_retains_text_and_refocuses_the_field_without_stored_row() {
    let (state, _) = state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Name")
        .type_text("  exact  ");
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    harness.run_steps(2);
    let proposed = vec![InsertInput::Value(EditInput {
        text: "  exact  ".into(),
        is_null: false,
    })];
    assert!(
        matches!(harness.state().1.as_slice(), [Action::ReviewInsert { draft_id: 20, inputs }] if inputs == &proposed)
    );
    harness.state_mut().0.begin_insert_review(20, proposed, 2);
    harness
        .state_mut()
        .0
        .fail_insert(2, "Native validation rejected this row".into());
    harness.run_steps(2);
    let input = harness.get_by_role_and_label(Role::TextInput, "Name");
    assert!(input.is_focused());
    assert_eq!(input.accesskit_node().value().as_deref(), Some("  exact  "));
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn retained_omitted_text_survives_navigation_refusal_and_stale_source() {
    let (mut state, _) = state();
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0].text = "retained".into();
    }
    state.select_view(super::super::DocumentView::Query);
    assert!(state.view == super::super::DocumentView::Table);
    state.show_open();
    assert!(!state.backstage_visible());
    state.clear_schema();
    assert!(state.data.is_some());
    state.database.as_mut().unwrap().document_revision += 1;
    state.invalidate_insert();
    assert!(
        matches!(&state.insertion, Some(Dialog::Draft { entries, error: Some(_), .. }) if entries[0].text == "retained")
    );
    assert!(!state.can_review_insert(20, &[InsertInput::Omitted]));
}

fn two_field_state() -> (State, InsertTarget) {
    state_with_fields(
        ["First", "Second"]
            .into_iter()
            .map(|name| InsertField {
                name: name.into(),
                input_hint: crate::InputHint::Text,
                nullable: true,
                auto_number: false,
            })
            .collect(),
    )
}

#[test]
fn inline_tab_moves_between_fields_without_publishing_or_reinterpreting_input() {
    let (state, _) = two_field_state();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "First")
        .type_text("  001  ");
    harness.run_steps(2);
    harness.key_press(egui::Key::Tab);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Second")
            .is_focused()
    );
    harness
        .get_by_role_and_label(Role::TextInput, "Second")
        .type_text("second value");
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run_steps(2);
    let first = harness.get_by_role_and_label(Role::TextInput, "First");
    assert!(first.is_focused());
    assert_eq!(first.accesskit_node().value().as_deref(), Some("  001  "));
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn inline_editor_scalar_limit_requires_a_real_correction_without_losing_hidden_text() {
    let (mut state, _) = two_field_state();
    if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
        entries[0].text = "retained".into();
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Second")
        .focus();
    harness.run_steps(2);
    harness.event(egui::Event::Paste(
        "x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1),
    ));
    harness.run_steps(2);
    assert!(matches!(
        &harness.state().0.insertion,
        Some(Dialog::Draft { entries, .. })
            if entries[0].text == "retained"
                && entries[1].text.is_empty()
                && entries[1].input_rejected
    ));
    assert!(
        !harness
            .state()
            .0
            .can_review_insert(20, &[InsertInput::Omitted, InsertInput::Omitted])
    );
    assert!(harness.state().1.is_empty());

    // Typing into an omitted field replaces its hidden cache; correcting the
    // rejected field is still necessary before review.
    harness
        .get_by_role_and_label(Role::TextInput, "First")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "First")
        .type_text("replacement");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Second")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "Second")
        .type_text("accepted");
    harness.run_steps(2);
    assert!(matches!(
        &harness.state().0.insertion,
        Some(Dialog::Draft { entries, .. })
            if entries[0].text == "replacement"
                && entries[1].text == "accepted"
                && !entries[1].input_rejected
    ));
    assert!(harness.state().1.is_empty());
    assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn same_frame_paste_and_review_uses_exact_accepted_text_and_rejects_oversized_input() {
    for oversized in [false, true] {
        let (state, _) = state();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 580.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .focus();
        harness.run_steps(1);
        let pasted = if oversized {
            "x".repeat(crate::text_input::MAX_EDITOR_SCALARS + 1)
        } else {
            "  exact pasted value  ".into()
        };
        harness.event(egui::Event::Paste(pasted.clone()));
        harness.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
        harness.event(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        });
        harness.run_steps(2);
        if oversized {
            assert!(harness.state().1.is_empty());
            assert!(
                matches!(&harness.state().0.insertion, Some(Dialog::Draft { entries, .. }) if entries[0].text.is_empty() && entries[0].input_rejected)
            );
            assert!(
                !harness
                    .state()
                    .0
                    .can_review_insert(20, &[InsertInput::Omitted])
            );
        } else {
            let expected = vec![InsertInput::Value(EditInput {
                text: pasted,
                is_null: false,
            })];
            assert!(
                matches!(harness.state().1.as_slice(), [Action::ReviewInsert { inputs, .. }] if inputs == &expected)
            );
        }
        assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
    }
}

#[test]
fn incompatible_same_revision_columns_preserve_visible_input_and_refuse_review_and_apply() {
    for duplicate in [false, true] {
        let (mut state, target) = state();
        if let Some(Dialog::Draft { entries, .. }) = &mut state.insertion {
            entries[0].choice = Choice::Value;
            entries[0].text = "retained exact value".into();
        }
        let proposed = vec![InsertInput::Value(EditInput {
            text: "retained exact value".into(),
            is_null: false,
        })];
        state.begin_insert_review(20, proposed.clone(), 2);
        assert!(state.set_insert_review(
            2,
            InsertReview {
                id: 30,
                draft_id: 20,
                target,
                inputs: proposed.clone(),
                inserted_rows: 1,
            }
        ));
        assert!(state.can_apply_insert(30));
        let data = state.data.as_mut().unwrap();
        if duplicate {
            data.columns.push("Name".into());
        } else {
            data.columns[0] = "Different".into();
        }
        assert!(!state.can_apply_insert(30));
        if let Some(Dialog::Draft { review, .. }) = &mut state.insertion {
            *review = None;
        }
        assert!(!state.can_review_insert(20, &proposed));
        state.begin_insert_review(20, proposed, 3);
        assert!(matches!(
            state.insertion,
            Some(Dialog::Draft { pending: None, .. })
        ));
        let mut harness = Harness::builder()
            .with_size(egui::vec2(900.0, 580.0))
            .build_ui_state(
                |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                (state, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        assert!(
            harness
                .get_all_by_value("retained exact value")
                .any(|node| node.accesskit_node().role() == Role::TextInput)
        );
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Review new row")
                .accesskit_node()
                .is_disabled()
        );
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .click_accesskit();
        harness.run_steps(2);
        assert!(matches!(
            harness.state().1.as_slice(),
            [Action::CloseInsert {
                draft_id: Some(20),
                ..
            }]
        ));
        assert!(harness.state().0.data.as_ref().unwrap().rows.is_empty());
    }
}

#[test]
fn many_individually_valid_editors_can_review_more_than_eight_mib_without_rendering_it() {
    let fields = (0..129)
        .map(|index| InsertField {
            name: format!("Memo{index}"),
            input_hint: InputHint::Text,
            nullable: true,
            auto_number: false,
        })
        .collect();
    let (mut state, _) = state_with_fields(fields);
    let Some(Dialog::Draft { entries, .. }) = &mut state.insertion else {
        panic!("expected a source-bound insertion draft");
    };
    for entry in entries.iter_mut() {
        entry.choice = Choice::Value;
        entry.text = "x".repeat(65_535);
    }
    assert!(entries.iter().map(|entry| entry.text.len()).sum::<usize>() > 8 * 1024 * 1024);
    let proposed = inputs(entries);
    assert!(state.can_review_insert(20, &proposed));
    assert!(state.data.as_ref().unwrap().rows.is_empty());
    state.begin_insert_review(20, proposed, 2);
    let Some(Dialog::Draft {
        pending: Some(pending),
        ..
    }) = &state.insertion
    else {
        panic!("valid editor values must reach reviewed native admission");
    };
    assert_eq!(pending.len(), 129);
    assert!(pending.iter().all(|input| matches!(input, InsertInput::Value(value) if value.text.len() == 65_535 && !value.is_null)));
}

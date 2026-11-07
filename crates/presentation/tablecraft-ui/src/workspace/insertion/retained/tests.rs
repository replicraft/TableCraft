use super::*;
use crate::workspace::insertion::{
    Choice, InsertReview,
    continuation::Seed,
    inputs,
    tests::{retained_entry, state_with_fields},
};
use crate::{
    Action, Data, Database, EditSaveOutcome, InputHint, InsertField, Locale, Localizer, Theme,
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn fields() -> Vec<InsertField> {
    vec![
        InsertField {
            name: "ID".into(),
            input_hint: InputHint::Int32,
            nullable: false,
            auto_number: true,
        },
        InsertField {
            name: "Name".into(),
            input_hint: InputHint::Text,
            nullable: true,
            auto_number: false,
        },
        InsertField {
            name: "Comment".into(),
            input_hint: InputHint::Text,
            nullable: true,
            auto_number: false,
        },
    ]
}

fn saving() -> State {
    let (mut state, target) = state_with_fields(fields());
    state.initial_record_notice = Some(target.clone());
    let Some(Dialog::Draft { entries, .. }) = &mut state.insertion else {
        panic!("native draft");
    };
    entries[1].choice = Choice::Omitted;
    entries[1].text = "  hidden name  ".into();
    entries[2].choice = Choice::Null;
    entries[2].text = "  hidden NULL text  ".into();
    let proposed = inputs(entries);
    state.begin_insert_review(20, proposed.clone(), 2);
    assert!(state.set_insert_review(
        2,
        InsertReview {
            id: 30,
            draft_id: 20,
            target,
            inputs: proposed,
            inserted_rows: 1,
        }
    ));
    state.begin_insert_save(30, 40);
    state
}

fn assert_snapshot(input: &RetainedInput) {
    assert_eq!(input.draft.fields.len(), 3);
    assert_eq!(input.draft.fields[0].name, "ID");
    assert!(input.draft.fields[0].auto_number);
    assert_eq!(input.entries[1].text, "  hidden name  ");
    assert!(input.entries[1].choice == Choice::Omitted);
    assert_eq!(input.entries[2].text, "  hidden NULL text  ");
    assert!(input.entries[2].choice == Choice::Null);
    assert!(input.initial_notice.is_some());
}

fn assert_continuation(state: &State) {
    let source = &state.insert_continuation.as_ref().unwrap().source;
    let Seed::Retained(input) = source else {
        panic!("retained native input");
    };
    assert_snapshot(input);
}

#[test]
fn resolution_and_stale_outcomes_preserve_original_metadata_presence_and_raw_text() {
    let mut state = saving();
    assert!(!state.can_acknowledge_insert_save(40));
    state.set_insert_save_outcome(
        40,
        EditSaveOutcome::Retained {
            message: "Unknown outcome".into(),
            can_recover: true,
            can_release: true,
        },
    );
    assert!(!state.close_insert());
    assert!(!state.acknowledge_insert_save(40, InsertRetryPermission::OriginalImage));
    state.begin_insert_resolution(40, 41);
    state.set_insert_save_outcome(40, EditSaveOutcome::Saved("Stale".into()));
    let Some(Dialog::Saving {
        save_id,
        retained: Some(input),
        ..
    }) = &state.insertion
    else {
        panic!("same retained owner");
    };
    assert_eq!(*save_id, 41);
    assert_snapshot(input);
    state.set_insert_save_outcome(
        41,
        EditSaveOutcome::Failed("No writes is only display text".into()),
    );
    assert!(state.can_acknowledge_insert_save(41));
    assert!(state.acknowledge_insert_save(41, InsertRetryPermission::Blocked));
    assert_continuation(&state);
    assert!(!state.can_retry_insert(41));
    assert!(!state.begin_insert_retry(41, 70));
    state.begin_insert(
        InsertTarget {
            document_revision: 7,
            table: "Empty table".into(),
        },
        99,
    );
    assert!(
        state.insertion.is_none(),
        "ordinary BeginInsert cannot bypass guarded retry"
    );
    assert!(!state.discard_retained_insert(40));
    assert!(state.discard_retained_insert(41));
}

#[test]
fn original_image_permission_keeps_even_saved_presentation_and_requires_fresh_guarded_metadata() {
    let mut state = saving();
    state.set_insert_save_outcome(
        40,
        EditSaveOutcome::Saved("Original verified, not inserted".into()),
    );
    assert!(state.acknowledge_insert_save(40, InsertRetryPermission::OriginalImage));
    assert_continuation(&state);
    assert!(state.can_retry_insert(40));
    assert!(state.begin_insert_retry(40, 70));
    assert!(!state.begin_insert_retry(40, 71));
    assert!(!state.discard_retained_insert(40));
    assert!(!state.fail_insert_retry(69, "stale".into()));
    let database = Database {
        document_revision: 8,
        title_path: None,
        title: "Reopened".into(),
        format: "ACE 16".into(),
        tables: vec![crate::Table {
            name: "Empty table".into(),
            local: true,
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    };
    let target = state.complete_insert_retry(70, database).unwrap();
    assert_eq!(target.document_revision, 8);
    assert_continuation(&state);
    state.set_table_data(Data {
        source_revision: Some(8),
        row_ordinals: None,
        table: "Empty table".into(),
        offset: 0,
        columns: fields()
            .iter()
            .map(|field| field.name.clone().into())
            .collect(),
        rows: Vec::new(),
        has_more: false,
    });
    let target = InsertTarget {
        document_revision: 8,
        table: "Empty table".into(),
    };
    state.begin_insert(target.clone(), 90);
    state.set_insert_draft(
        90,
        InsertDraft {
            id: 91,
            target: target.clone(),
            fields: fields(),
        },
    );
    assert!(!state.insert_continuation_pending());
    assert_eq!(
        retained_entry(&state, 1),
        (super::super::InsertInput::Omitted, "  hidden name  ")
    );
    assert_eq!(retained_entry(&state, 2).1, "  hidden NULL text  ");
    assert_eq!(state.initial_record_notice, Some(target));
    assert!(!state.can_apply_insert(30));
}

#[test]
fn cancelling_guarded_reopen_preserves_copyable_input_and_requires_another_explicit_attempt() {
    let mut state = saving();
    state.set_insert_save_outcome(
        40,
        EditSaveOutcome::Cancelled("Terminal resources acknowledged separately".into()),
    );
    assert!(state.acknowledge_insert_save(40, InsertRetryPermission::OriginalImage));
    assert!(state.begin_insert_retry(40, 70));
    assert!(state.fail_insert_retry(70, "Read cancelled".into()));
    assert_continuation(&state);
    assert!(state.can_retry_insert(40));
    assert!(!state.fail_insert_retry(70, "late response".into()));
}

#[test]
fn retained_input_is_read_only_in_all_locales_and_themes_until_explicit_discard() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let mut state = saving();
            state.localizer = Localizer::new(locale).unwrap();
            state.theme = theme;
            state
                .set_insert_save_outcome(40, EditSaveOutcome::Failed("Unqualified outcome".into()));
            assert!(state.acknowledge_insert_save(40, InsertRetryPermission::Blocked));
            let mut harness = Harness::builder()
                .with_size(egui::vec2(900.0, 580.0))
                .build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
            crate::configure(&harness.ctx);
            harness.run_steps(2);
            let retry = harness.state().0.localizer.text("insertion-retry-original");
            assert!(
                harness
                    .get_by_role_and_label(Role::Button, &retry)
                    .accesskit_node()
                    .is_disabled()
            );
            let copy = harness.get_by_role_and_label(Role::MultilineTextInput, "Name");
            assert!(copy.accesskit_node().data().is_read_only());
            for action in [
                egui::accesskit::Action::SetValue,
                egui::accesskit::Action::ReplaceSelectedText,
            ] {
                assert!(!copy.accesskit_node().data().supports_action(action));
            }
            copy.focus();
            harness.run_steps(1);
            harness
                .input_mut()
                .events
                .push(egui::Event::Text("Must not change retained text".into()));
            harness.run_steps(2);
            assert_continuation(&harness.state().0);
            assert!(harness.state().1.is_empty());
            harness.get_by_label("Unqualified outcome");
            assert_eq!(
                harness.state().0.localizer.diagnostics(),
                Default::default()
            );
        }
    }
}

#[test]
fn large_hidden_input_has_bounded_preview_and_explicit_copy_keeps_every_byte() {
    let mut state = saving();
    let original = "é".repeat(40_000);
    let Some(Dialog::Saving {
        retained: Some(input),
        ..
    }) = &mut state.insertion
    else {
        panic!("save input owner");
    };
    input.entries[1].text = original.clone();
    state.set_insert_save_outcome(40, EditSaveOutcome::Failed("Unqualified".into()));
    assert!(state.acknowledge_insert_save(40, InsertRetryPermission::Blocked));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.show(ui);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let preview = harness.get_by_role_and_label(Role::MultilineTextInput, "Name");
    let preview_text = preview.accesskit_node().value().unwrap();
    assert_eq!(preview_text.chars().count(), 512);
    let shortened = harness
        .state()
        .localizer
        .text("insertion-preview-truncated");
    assert!(harness.get_all_by_label(&shortened).count() > 0);
    harness
        .get_all_by_role_and_label(Role::Button, "Copy complete input")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    assert!(
        harness
            .output()
            .platform_output
            .commands
            .iter()
            .any(|command| {
                matches!(command, egui::OutputCommand::CopyText(text) if text == &original)
            })
    );
}

#[test]
fn long_terminal_diagnostic_stays_inside_scroll_body_with_footer_visible() {
    let mut state = saving();
    state.set_insert_save_outcome(40, EditSaveOutcome::Failed("Long diagnosis ".repeat(400)));
    assert!(state.acknowledge_insert_save(40, InsertRetryPermission::Blocked));
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut State| {
                state.show(ui);
            },
            state,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    for key in ["insertion-retry-original", "insertion-discard-retained"] {
        let label = harness.state().localizer.text(key);
        let rect = harness.get_by_role_and_label(Role::Button, &label).rect();
        assert!(
            rect.top() >= 0.0 && rect.bottom() <= 580.0,
            "{key}: {rect:?}"
        );
    }
}

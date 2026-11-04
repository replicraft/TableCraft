use super::*;
use crate::workspace::{Action, Database};

fn state() -> (State, EditTarget) {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
    });
    state.set_table_data(Data {
        source_revision: Some(7),
        table: "Items".into(),
        offset: 100,
        columns: vec!["Amount".into()],
        rows: vec![vec!["12".into()]],
        has_more: false,
    });
    let target = state.data.as_ref().unwrap().edit_target(0, 0).unwrap();
    (state, target)
}

fn draft(target: EditTarget) -> EditDraft {
    EditDraft {
        id: 20,
        target,
        original: "12".into(),
        initial: EditInput {
            text: "12".into(),
            is_null: false,
        },
        input_kind: "Signed integer".into(),
        nullable: true,
    }
}

fn review(target: EditTarget) -> EditReview {
    EditReview {
        id: 30,
        draft_id: 20,
        target,
        before: "12".into(),
        after: "13".into(),
        changed_rows: 1,
    }
}

#[test]
fn review_binds_exact_input_and_closed_or_replaced_drafts_ignore_late_replies() {
    let (mut state, target) = state();
    assert_eq!(target.parent_ordinal, 100);
    state.begin_edit(target.clone(), 1);
    state.set_edit_draft(1, draft(target.clone()));
    let input = EditInput {
        text: "13".into(),
        is_null: false,
    };
    if let Some(EditDialog::Draft { input: current, .. }) = &mut state.edit {
        *current = input.clone();
    }
    state.begin_edit_review(20, input, 2);
    assert!(state.set_edit_review(2, review(target.clone())));
    assert!(state.can_apply_edit(30));
    assert!(!state.can_apply_edit(31));
    if let Some(EditDialog::Draft { input, .. }) = &mut state.edit {
        input.text = "14".into();
    }
    assert!(!state.can_apply_edit(30));
    if let Some(EditDialog::Draft { input, .. }) = &mut state.edit {
        input.text = "13".into();
    }
    assert!(state.can_apply_edit(30));
    state.fail_edit_apply(30, "Save worker is busy".into());
    assert!(matches!(
        &state.edit,
        Some(EditDialog::Draft {
            review: Some(_),
            ..
        })
    ));
    state.select_view(DocumentView::Query);
    state.set_edit_review(2, review(target.clone()));
    assert!(!state.edit_dialog_open());
    state.select_view(DocumentView::Table);
    state.begin_edit(target.clone(), 3);
    state.close_edit();
    state.set_edit_draft(3, draft(target));
    assert!(!state.edit_dialog_open());
}

#[test]
fn saving_and_retained_failure_block_close_and_navigation_until_actual_release() {
    let (mut state, target) = state();
    state.begin_edit(target.clone(), 1);
    state.set_edit_draft(1, draft(target.clone()));
    let input = EditInput {
        text: "12".into(),
        is_null: false,
    };
    state.begin_edit_review(20, input, 2);
    state.set_edit_review(2, review(target));
    state.begin_edit_save(30, 40);
    assert!(!state.close_edit());
    state.show_open();
    assert!(!state.backstage_visible());
    state.select_view(DocumentView::Query);
    assert!(state.view == DocumentView::Table);
    state.set_edit_save_outcome(
        40,
        EditSaveOutcome::Retained {
            message: "Recovery required".into(),
            can_recover: true,
            can_release: false,
        },
    );
    assert!(!state.close_edit());
    state.begin_edit_resolution(40, 41);
    state.set_edit_save_outcome(
        41,
        EditSaveOutcome::Saved("Saved and resources released".into()),
    );
    assert!(state.close_edit());
    assert!(!state.edit_dialog_open());
}

#[test]
fn edit_modal_owns_escape_without_running_hidden_query_or_claiming_rollback() {
    let (mut state, target) = state();
    state.begin_edit(target, 1);
    let ctx = egui::Context::default();
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 820.0),
            )),
            ..Default::default()
        },
        |ui| actions = state.show(ui),
    );
    output.textures_delta.clear();
    assert!(actions.is_empty());
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 820.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ui| actions = state.show(ui),
    );
    output.textures_delta.clear();
    assert!(matches!(
        actions.as_slice(),
        [Action::CloseEdit {
            request_id: 1,
            draft_id: None
        }]
    ));
    assert!(!state.edit_dialog_open());
}

#[test]
fn only_native_ordinary_scalar_candidates_have_edit_targets() {
    let (mut state, target) = state();
    assert!(state.can_edit_cell(&target));
    let data = state.data.as_mut().unwrap();
    data.source_revision = None;
    assert!(data.edit_target(0, 0).is_none());
    data.source_revision = Some(7);
    data.rows[0][0].kind = CellKind::Scalar {
        is_null: false,
        stored_calculation: true,
        boolean: None,
        display_control: crate::DisplayControl::Uninspected,
    };
    assert!(data.edit_target(0, 0).is_none());
    data.rows[0][0].kind = CellKind::Attachments {
        assigned: true,
        members: 1,
    };
    assert!(data.edit_target(0, 0).is_none());
    assert!(!state.can_edit_cell(&target));
}

#[test]
fn edited_input_rejects_late_review_and_validation_error_preserves_replacement() {
    let (mut state, target) = state();
    state.begin_edit(target.clone(), 1);
    state.set_edit_draft(1, draft(target.clone()));
    state.begin_edit_review(
        20,
        EditInput {
            text: "12".into(),
            is_null: false,
        },
        2,
    );
    if let Some(EditDialog::Draft { input, .. }) = &mut state.edit {
        input.text = "not an integer".into();
    }
    state.set_edit_review(2, review(target));
    assert!(matches!(
        &state.edit,
        Some(EditDialog::Draft { review: None, .. })
    ));
    state.fail_edit(2, "Replacement must be a signed integer".into());
    let Some(EditDialog::Draft { input, error, .. }) = &state.edit else {
        panic!("draft remains open");
    };
    assert_eq!(input.text, "not an integer");
    assert!(error.is_some());
}

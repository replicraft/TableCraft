use super::*;
use crate::workspace::{Action, Database};

fn state() -> (State, EditTarget) {
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(Database {
        title_path: None,
        document_revision: 7,
        title: "Example".into(),
        format: "ACE16".into(),
        queries: vec![],
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

#[test]
fn compact_edit_dialog_keeps_actions_visible_beside_long_literal_content() {
    let literal =
        "Literal service detail and replacement text\n".repeat(100) + "FINAL EDIT DIAGNOSIS";
    for variant in 0..4 {
        let (mut state, target) = state();
        state.begin_edit(target.clone(), 1);
        let expected = match variant {
            0 => {
                state.fail_edit(1, literal.clone());
                "Cancel"
            }
            1 => {
                let mut draft = draft(target);
                draft.initial.text = literal.clone();
                state.set_edit_draft(1, draft);
                "Review change"
            }
            _ => {
                state.edit = Some(EditDialog::Finished {
                    save_id: 40,
                    outcome: if variant == 2 {
                        EditSaveOutcome::Failed(literal.clone())
                    } else {
                        EditSaveOutcome::Retained {
                            message: literal.clone(),
                            can_recover: true,
                            can_release: false,
                        }
                    },
                });
                if variant == 2 {
                    "Close"
                } else {
                    "Recover save"
                }
            }
        };
        let context = egui::Context::default();
        context.enable_accesskit();
        for frame in 0..12 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 580.0),
                    )),
                    time: Some(f64::from(frame) * 0.1),
                    events: if frame >= 4 {
                        vec![
                            egui::Event::PointerMoved(egui::pos2(450.0, 280.0)),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(0.0, -10000.0),
                                modifiers: egui::Modifiers::NONE,
                                phase: egui::TouchPhase::Move,
                            },
                        ]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ui| assert!(state.show(ui).is_empty()),
            );
            if frame == 3 || frame == 11 {
                let tree = output.platform_output.accesskit_update.as_ref().unwrap();
                let button = tree
                    .nodes
                    .iter()
                    .find_map(|(_, node)| {
                        (node.role() == egui::accesskit::Role::Button
                            && node.label() == Some(expected))
                        .then(|| node.bounds())
                        .flatten()
                    })
                    .unwrap_or_else(|| panic!("missing action {expected}"));
                assert!(
                    button.y0 >= 0.0 && button.y1 <= 580.0,
                    "action outside compact viewport: {button:?}"
                );
                let (text, clip) = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        let egui::epaint::Shape::Text(text) = &shape.shape else {
                            return None;
                        };
                        (text.galley.job.text == literal).then_some((text, shape.clip_rect))
                    })
                    .expect("full literal content retained inside its scroll region");
                let final_row = text
                    .galley
                    .rows
                    .last()
                    .unwrap()
                    .rect()
                    .translate(text.pos.to_vec2());
                if frame == 3 {
                    assert!(
                        !clip.contains_rect(final_row),
                        "diagnosis initially extends beyond body"
                    );
                } else {
                    assert!(
                        clip.contains_rect(final_row),
                        "wheel reveals final unique diagnosis line: variant={variant}, row={final_row:?}, clip={clip:?}"
                    );
                }
            }
            output.textures_delta.clear();
        }
        if variant == 3 {
            assert!(state.edit_save_pending());
            assert!(!state.close_edit());
        }
    }
}

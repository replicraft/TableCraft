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
        row_ordinals: None,
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
        input_hint: InputHint::Int32,
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

#[test]
fn german_and_japanese_input_hints_render_in_cell_and_reviewed_row_forms_without_changing_inputs() {
    for (locale, hint, expected) in [
        (
            crate::Locale::De,
            InputHint::Currency,
            r#"Währung: Dezimalbetrag (vier Nachkommastellen) Als Dezimaltrennzeichen "." verwenden."#,
        ),
        (
            crate::Locale::Ja,
            InputHint::DateTimeExtendedIso,
            "拡張日時: ISO形式のローカル日時（100 ns精度）",
        ),
    ] {
        for insertion in [false, true] {
            let (mut state, target) = state();
            state.localizer = crate::Localizer::new(locale).unwrap();
            if insertion {
                state.database.as_mut().unwrap().tables.push(crate::Table {
                    local: true,
                    name: target.table.clone(),
                    system: false,
                    storage: "Local".into(),
                });
                let target = crate::InsertTarget {
                    document_revision: target.document_revision,
                    table: target.table,
                };
                state.begin_insert(target.clone(), 1);
                state.set_insert_draft(
                    1,
                    crate::InsertDraft {
                        id: 20,
                        target,
                        fields: vec![crate::InsertField {
                            name: "Amount".into(),
                            input_hint: hint,
                            nullable: true,
                            auto_number: false,
                        }],
                    },
                );
                state.begin_insert_review(20, vec![crate::InsertInput::Omitted], 2);
            } else {
                state.begin_edit(target.clone(), 1);
                let mut draft = draft(target);
                draft.input_hint = hint;
                state.set_edit_draft(1, draft);
            }

            let context = egui::Context::default();
            let mut hint_visible = false;
            for _ in 0..2 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(900.0, 580.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        assert!(state.show(ui).is_empty());
                    },
                );
                hint_visible |= output.shapes.iter().any(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => {
                        let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                        text.galley.job.text == expected && shape.clip_rect.contains_rect(bounds)
                    }
                    _ => false,
                });
                output.textures_delta.clear();
            }
            assert!(hint_visible, "{locale:?}, insertion={insertion}");
            if let Some(EditDialog::Draft { input, .. }) = &state.edit {
                assert_eq!(input.text, "12");
                assert!(!input.is_null);
            }
            assert_eq!(state.localizer.diagnostics(), Default::default());
        }
    }
}

#[test]
fn literal_paste_seeds_only_matching_draft_without_review_or_null_conversion() {
    for text in ["", "NULL", "first\tsecond\nnext line"] {
        let (mut state, target) = state();
        assert!(state.begin_paste_edit(target.clone(), text.into(), 1));
        state.set_edit_draft(9, draft(target.clone()));
        assert!(matches!(state.edit, Some(EditDialog::Checking { .. })));

        state.set_edit_draft(1, draft(target));
        let Some(EditDialog::Draft {
            input,
            review,
            pending,
            ..
        }) = &state.edit
        else {
            panic!("matching draft receives literal text");
        };
        assert_eq!(input.text, text);
        assert!(!input.is_null);
        assert!(review.is_none());
        assert!(pending.is_none());
    }

    let (mut state, target) = state();
    let oversized = "é".repeat(512 * 1024 + 1);
    assert!(!state.begin_paste_edit(target.clone(), oversized, 1));
    assert!(!state.edit_dialog_open());
    assert!(state.error.as_ref().unwrap().contains("1 MiB"));

    state.begin_edit(target.clone(), 2);
    state.set_edit_draft(2, draft(target));
    let Some(EditDialog::Draft { input, .. }) = &state.edit else {
        panic!("ordinary draft remains available");
    };
    assert_eq!(input.text, "12");
}

#[test]
fn home_clipboard_witness_rejects_late_source_focus_and_dialog_changes() {
    for change in ["source", "focus", "dialog"] {
        let (mut state, target) = state();
        let context = egui::Context::default();
        state.request_paste(&context);
        match change {
            "source" => {
                state.set_table_data(Data {
                    source_revision: Some(7),
                    row_ordinals: None,
                    table: "Replacement".into(),
                    offset: 100,
                    columns: vec!["Amount".into()],
                    rows: vec![vec!["99".into()]],
                    has_more: false,
                });
            }
            "focus" => context.memory_mut(|memory| {
                memory.request_focus(egui::Id::new("newer_text_editor"));
            }),
            "dialog" => state.begin_edit(target.clone(), 2),
            _ => unreachable!(),
        }
        let mut actions = vec![Action::PasteCell {
            target,
            text: "late".into(),
        }];
        state.finish_paste(&context, &mut actions, None);
        assert!(actions.is_empty(), "{change} invalidates clipboard target");
        assert!(
            state.paste_request.is_some(),
            "retain unanswered OS request"
        );
        let mut output = context.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Paste("delayed OS clipboard".into())],
                ..Default::default()
            },
            |ui| {
                state.receive_requested_paste(ui.ctx(), &mut actions);
            },
        );
        output.textures_delta.clear();
        assert!(
            actions.is_empty(),
            "late clipboard must not target newer focus"
        );
        assert!(state.paste_request.is_none());
        assert!(state.error.is_some());
    }
}

#[test]
fn received_home_paste_rechecks_same_frame_focus_and_rejects_duplicate_payloads() {
    for duplicate in [false, true] {
        let (mut state, _) = state();
        let context = egui::Context::default();
        state.request_paste(&context);
        let mut events = vec![egui::Event::Paste("first payload".into())];
        if duplicate {
            events.push(egui::Event::Paste("second payload".into()));
        }
        let mut actions = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                let receipt = state.receive_requested_paste(ui.ctx(), &mut actions);
                assert!(!ui.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, egui::Event::Paste(_)))
                }));
                ui.memory_mut(|memory| {
                    memory.request_focus(egui::Id::new("newer_navigation_search"));
                });
                state.finish_paste(ui.ctx(), &mut actions, receipt);
            },
        );
        output.textures_delta.clear();
        assert!(actions.is_empty());
        assert!(state.error.is_some());
        assert!(state.paste_request.is_none());
    }
}

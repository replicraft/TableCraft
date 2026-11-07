use super::*;
use crate::Action;

#[test]
fn pending_mock_saves_render_without_claiming_completion_or_releasing_the_owner() {
    for scene in [
        Scene::SaveApplying,
        Scene::SaveCancelling,
        Scene::SaveResolving,
    ] {
        let mut preview = Preview::new(scene);
        assert!(preview.state.mock_preview);
        assert!(preview.state.edit_save_pending());
        assert!(!preview.state.close_edit());
        let expected_cancel = scene == Scene::SaveCancelling;
        let expected_resolving = scene == Scene::SaveResolving;
        assert!(matches!(&preview.state.edit,
            Some(EditDialog::Saving { cancel_requested, resolving, .. })
                if *cancel_requested == expected_cancel && *resolving == expected_resolving));

        let context = egui::Context::default();
        crate::configure(&context);
        for _ in 0..3 {
            render(&mut preview, &context);
            assert!(preview.state.edit_save_pending());
            assert!(matches!(
                preview.state.edit,
                Some(EditDialog::Saving { .. })
            ));
        }
        assert_eq!(preview.state.data.as_ref().unwrap().table, "Customers");
        assert_eq!(
            preview.state.data.as_ref().unwrap().source_revision,
            Some(1)
        );
    }
}

#[test]
fn mock_review_rejection_drops_the_review_and_terminal_diagnosis_cannot_be_dismissed() {
    let preview = Preview::new(Scene::EditRejected);
    assert!(matches!(&preview.state.edit,
        Some(EditDialog::Draft { pending: None, review: None, error: Some(error), input, .. })
            if !error.is_empty() && input.text == "1495.50"));

    for scene in [
        Scene::SaveSaved,
        Scene::SaveCancelled,
        Scene::SaveFailed,
        Scene::RecoveryDiagnosis,
    ] {
        let mut preview = Preview::new(scene);
        let heading = match scene {
            Scene::SaveSaved => Some("Save completed"),
            Scene::SaveCancelled => Some("Save cancelled"),
            Scene::SaveFailed => Some("Save needs attention"),
            Scene::RecoveryDiagnosis => None,
            _ => unreachable!("only terminal scenes are supplied"),
        };
        assert_terminal_presentation(&mut preview, heading);
        let outcome = match &preview.state.edit {
            Some(EditDialog::Finished { outcome, .. }) => outcome,
            _ => panic!("expected synthetic terminal outcome"),
        };
        match (scene, outcome) {
            (Scene::SaveSaved, EditSaveOutcome::Saved(message))
            | (Scene::SaveCancelled, EditSaveOutcome::Cancelled(message))
            | (Scene::SaveFailed, EditSaveOutcome::Failed(message)) => {
                assert!(!message.is_empty());
                assert!(!preview.state.edit_save_pending());
                assert!(preview.state.close_edit());
            }
            (
                Scene::RecoveryDiagnosis,
                EditSaveOutcome::Retained {
                    can_recover,
                    can_release,
                    ..
                },
            ) => {
                assert!(!can_recover);
                assert!(!can_release);
                assert!(preview.state.edit_save_pending());
                assert!(!preview.state.close_edit());
            }
            _ => panic!("scene must retain its exact mock outcome kind"),
        }
    }

    const KNOWN_RECEIPTS: &str = "Current operation: resource release failed. Established outcome: commit completed, 2 rows changed. Prior operation: validation failed.";
    let mut preview = Preview::new(Scene::SaveFailed);
    let save_id = match &preview.state.edit {
        Some(EditDialog::Finished { save_id, .. }) => *save_id,
        _ => panic!("expected the terminal save fixture"),
    };
    preview
        .state
        .set_edit_save_outcome(save_id, EditSaveOutcome::Failed(KNOWN_RECEIPTS.into()));
    assert_terminal_presentation(&mut preview, Some("Save needs attention"));
    assert!(matches!(&preview.state.edit,
        Some(EditDialog::Finished { outcome: EditSaveOutcome::Failed(message), .. })
            if message == KNOWN_RECEIPTS));
}

fn render(preview: &mut Preview, context: &egui::Context) -> egui::FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 820.0),
            )),
            ..Default::default()
        },
        |ui| assert!(preview.state.show(ui).is_empty()),
    );
    output.textures_delta.clear();
    output
}

fn assert_terminal_presentation(preview: &mut Preview, expected_heading: Option<&str>) {
    let original_message = match &preview.state.edit {
        Some(EditDialog::Finished { outcome, .. }) => match outcome {
            EditSaveOutcome::Saved(message)
            | EditSaveOutcome::Cancelled(message)
            | EditSaveOutcome::Failed(message)
            | EditSaveOutcome::Retained { message, .. } => message.clone(),
        },
        _ => panic!("expected a terminal fixture"),
    };
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut last = None;
    for _ in 0..3 {
        last = Some(render(preview, &context));
    }
    let output = last.expect("settled terminal frame");
    let painted = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();

    assert!(
        painted.contains(&original_message.as_str()),
        "host message stays literal"
    );
    for heading in [
        "Save completed",
        "Save cancelled",
        "Save needs attention",
        "Save failed",
        "No changes were made",
        "Changes were rolled back",
    ] {
        assert_eq!(
            painted.contains(&heading),
            expected_heading == Some(heading)
        );
        assert_eq!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some(heading) || node.value() == Some(heading)),
            expected_heading == Some(heading),
            "the actual accessible outcome must match the terminal enum",
        );
    }
    assert!(!painted.iter().any(|text| text.starts_with("Changed rows:")));
    assert!(!tree.nodes.iter().any(|(_, node)| {
        node.role() == egui::accesskit::Role::Button
            && matches!(
                node.label(),
                Some("Recover save" | "Release save resources")
            )
    }));
}

#[test]
fn mock_release_preserves_needs_attention_without_claiming_a_save() {
    let mut preview = Preview::new(Scene::Recovery);
    let Some(EditDialog::Finished {
        save_id,
        outcome: EditSaveOutcome::Retained {
            can_release: true, ..
        },
    }) = &preview.state.edit
    else {
        panic!("retained mock resources can be released explicitly");
    };
    let save_id = *save_id;
    let rows = preview.state.data.as_ref().unwrap().rows.as_ptr();
    preview.dispatch(Action::ReleaseEditSave { save_id });
    assert!(
        matches!(&preview.state.edit, Some(EditDialog::Finished { outcome: EditSaveOutcome::Failed(message), .. })
        if message == &preview.state.localizer.text("settings-mock-resolved"))
    );
    assert_terminal_presentation(&mut preview, Some("Save needs attention"));
    assert_eq!(preview.state.data.as_ref().unwrap().rows.as_ptr(), rows);
    assert!(!preview.state.edit_save_pending());
    assert!(preview.state.close_edit());
    assert!(preview.state.mock_preview);
}

#[test]
fn save_owner_refuses_stale_actions_and_commands_until_current_terminal_acknowledgment() {
    for scene in [
        Scene::SaveApplying,
        Scene::SaveSaved,
        Scene::SaveCancelled,
        Scene::SaveFailed,
        Scene::RecoveryDiagnosis,
    ] {
        let seed = Preview::new(scene);
        let save_id = match &seed.state.edit {
            Some(EditDialog::Saving { save_id, .. } | EditDialog::Finished { save_id, .. }) => {
                *save_id
            }
            _ => panic!("save fixture retains its owner"),
        };
        for action in [
            Action::Open,
            Action::ChooseFile,
            Action::Page {
                table: "Orders".into(),
                offset: 0,
            },
            Action::ViewSchema {
                table: "Orders".into(),
            },
            Action::Query,
            Action::CancelEditSave {
                save_id: save_id + 1,
            },
            Action::RecoverEditSave { save_id },
            Action::ReleaseEditSave { save_id },
            Action::AcknowledgeEditSave {
                save_id: save_id + 1,
            },
            Action::CloseEdit {
                request_id: save_id,
                draft_id: None,
            },
        ] {
            let mut preview = Preview::new(scene);
            preview.state.error = Some("Existing report".into());
            preview.state.password = "synthetic password".into();
            let rows = preview.state.data.as_ref().unwrap().rows.as_ptr();
            let before = std::mem::discriminant(preview.state.edit.as_ref().unwrap());
            preview.dispatch(action);
            let current = preview
                .state
                .edit
                .as_ref()
                .expect("stale action cannot drop owner");
            assert_eq!(std::mem::discriminant(current), before);
            assert!(
                matches!(current, EditDialog::Saving { save_id: current, .. } | EditDialog::Finished { save_id: current, .. } if *current == save_id)
            );
            assert_eq!(preview.state.data.as_ref().unwrap().rows.as_ptr(), rows);
            assert_eq!(preview.state.error.as_deref(), Some("Existing report"));
            assert_eq!(preview.state.password, "synthetic password");
        }
        if scene != Scene::SaveApplying && scene != Scene::RecoveryDiagnosis {
            let mut preview = Preview::new(scene);
            preview.dispatch(Action::AcknowledgeEditSave { save_id });
            assert!(!preview.state.edit_dialog_open());
        }
    }
}

#[test]
fn mock_cancel_requests_remain_pending_and_late_requests_preserve_finished_outcomes() {
    for scene in [Scene::SaveApplying, Scene::SaveResolving] {
        let mut preview = Preview::new(scene);
        let save_id = match &preview.state.edit {
            Some(EditDialog::Saving { save_id, .. }) => *save_id,
            _ => panic!("pending save fixture"),
        };
        preview.dispatch(Action::CancelEditSave { save_id });
        assert!(
            matches!(&preview.state.edit, Some(EditDialog::Saving { save_id: current, cancel_requested: true, .. }) if *current == save_id)
        );
        assert!(preview.state.edit_save_pending());
        assert!(!preview.state.close_edit());
        preview.state.set_edit_save_outcome(
            save_id,
            EditSaveOutcome::Saved("Established mock completion".into()),
        );
        preview.dispatch(Action::CancelEditSave { save_id });
        preview.dispatch(Action::Cancel);
        assert!(
            matches!(&preview.state.edit, Some(EditDialog::Finished { outcome: EditSaveOutcome::Saved(message), .. }) if message == "Established mock completion")
        );
        assert!(preview.state.error.is_none());
    }
}

#[test]
fn stale_edit_close_cannot_dismiss_a_newer_draft() {
    let mut preview = Preview::new(Scene::Edit);
    let (request_id, draft_id) = match &preview.state.edit {
        Some(EditDialog::Draft {
            request_id, draft, ..
        }) => (*request_id, draft.id),
        _ => panic!("editable draft fixture"),
    };
    for action in [
        Action::CloseEdit {
            request_id: request_id + 1,
            draft_id: Some(draft_id),
        },
        Action::CloseEdit {
            request_id,
            draft_id: Some(draft_id + 1),
        },
        Action::AcknowledgeEditSave {
            save_id: request_id,
        },
    ] {
        preview.dispatch(action);
        assert!(preview.state.edit_dialog_open());
    }
    preview.dispatch(Action::CloseEdit {
        request_id,
        draft_id: Some(draft_id),
    });
    assert!(!preview.state.edit_dialog_open());
}

#[test]
fn retained_journal_preview_renders_all_locales_and_themes_without_changing_ordinary_saved_scene() {
    for locale in crate::Locale::ALL {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let mut preview = Preview::new(Scene::SaveSaved);
            preview.state.localizer = crate::Localizer::new(locale).unwrap();
            preview.state.theme = theme;
            preview.set_scene(Scene::SaveSaved);
            let ordinary = preview.state.localizer.text("settings-mock-saved");
            assert!(matches!(&preview.state.edit,
                Some(EditDialog::Finished { outcome: EditSaveOutcome::Saved(message), .. }) if *message == ordinary));
            assert_eq!(preview.state.save_cleanup_status(), None);

            preview.set_scene(Scene::SaveSavedRetainedJournal);
            assert!(preview.state.mock_preview);
            assert!(!preview.state.edit_save_pending());
            let notice = preview
                .state
                .localizer
                .text("editing-save-retained-private-journal");
            let expected = format!(
                "{} {}",
                preview.state.localizer.text("editing-save-completed"),
                notice
            );
            assert!(matches!(&preview.state.edit,
                Some(EditDialog::Finished { outcome: EditSaveOutcome::Saved(message), .. }) if *message == expected));
            let context = egui::Context::default();
            crate::configure(&context);
            let mut settled = None;
            for _ in 0..3 {
                settled = Some(render(&mut preview, &context));
            }
            let output = settled.unwrap();
            let painted = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(
                painted.contains(&notice.as_str()),
                "status must retain the cleanup fact: {locale:?} {theme:?}"
            );
            assert!(
                painted.contains(&expected.as_str()),
                "terminal dialog must retain the same fact: {locale:?} {theme:?}"
            );
            assert_eq!(preview.state.localizer.diagnostics(), Default::default());

            // A later document cannot inherit this status. Its historical
            // terminal report remains unchanged until explicitly dismissed.
            preview.state.database.as_mut().unwrap().document_revision += 1;
            assert_eq!(preview.state.save_cleanup_status(), None);
            assert!(matches!(&preview.state.edit,
                Some(EditDialog::Finished { outcome: EditSaveOutcome::Saved(message), .. }) if *message == expected));
            preview.set_scene(Scene::SaveSaved);
            assert_eq!(preview.state.save_cleanup_status(), None);
            assert!(matches!(&preview.state.edit,
                Some(EditDialog::Finished { outcome: EditSaveOutcome::Saved(message), .. }) if *message == ordinary));
        }
    }
}

use super::*;
use crate::{Action, EditSaveOutcome};

#[test]
fn every_existing_preview_scene_renders_without_a_runtime_or_file_host() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut preview = Preview::new(Scene::Datasheet);
    for locale in crate::Locale::ALL {
        preview.state_mut().localizer = crate::Localizer::new(locale).unwrap();
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            preview.state_mut().theme = theme;
            for scene in Scene::ALL {
                preview.set_scene(scene);
                assert!(preview.state().mock_preview);
                let empty_identity = if scene == Scene::Empty {
                    let data = preview.state.data.as_ref().unwrap();
                    Some((
                        data.source_revision,
                        data.table.clone(),
                        data.offset,
                        data.columns.clone(),
                    ))
                } else {
                    None
                };

                for _ in 0..2 {
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1280.0, 820.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            if scene == Scene::Empty {
                                assert!(preview.state.show(ui).is_empty());
                            } else {
                                preview.show(ui);
                            }
                        },
                    );
                    output.textures_delta.clear();
                    if let Some(identity) = &empty_identity {
                        let painted = output
                            .shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::epaint::Shape::Text(text) => {
                                    Some(text.galley.job.text.as_str())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        for label in ["Customer ID", "Customer", "Company"] {
                            assert!(
                                painted.contains(&label),
                                "missing declared empty-table header: {label}"
                            );
                        }
                        let notice = preview.state.localizer.text("datasheet-no-rows");
                        assert!(painted.contains(&notice.as_str()));

                        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
                        assert!(
                            !tree
                                .nodes
                                .iter()
                                .any(|(_, node)| node.role() == egui::accesskit::Role::RowHeader)
                        );
                        assert!(tree.nodes.iter().any(|(_, node)| {
                            node.description()
                                .is_some_and(|description| description.contains("CustomerID"))
                        }));

                        let data = preview.state.data.as_ref().unwrap();
                        assert_eq!(
                            &(
                                data.source_revision,
                                data.table.clone(),
                                data.offset,
                                data.columns.clone()
                            ),
                            identity
                        );
                        assert!(data.rows.is_empty());
                        assert!(!preview.state.can_copy_selection());
                    }
                    assert!(
                        !output.shapes.is_empty(),
                        "{locale}/{theme:?}/{} did not draw",
                        scene.as_str()
                    );
                }
                assert_eq!(
                    preview.state().localizer.diagnostics(),
                    crate::localization::LocalizationDiagnostics::default(),
                    "{locale}/{theme:?}/{} has localization diagnostics",
                    scene.as_str(),
                );
            }
        }
    }
}

#[test]
fn preview_open_never_reads_a_user_path_and_mock_recovery_is_only_presentation() {
    let mut preview = Preview::new(Scene::Backstage);
    preview.state.path = "/a/user/path/that/the/preview/must/not/open.accdb".into();
    preview.state.password = "not a database credential".into();
    preview.dispatch(Action::Open);
    assert!(preview.state.password.is_empty());
    assert!(preview.state.error.as_deref().unwrap().contains("disabled"));
    assert_eq!(
        preview.state.database.as_ref().unwrap().document_revision,
        1
    );

    preview.set_scene(Scene::Recovery);
    let Some(super::super::editing::EditDialog::Finished { save_id, outcome }) =
        &preview.state.edit
    else {
        panic!("expected synthetic retained outcome");
    };
    assert!(matches!(outcome, EditSaveOutcome::Retained { .. }));
    let save_id = *save_id;
    preview.dispatch(Action::RecoverEditSave { save_id });
    assert!(matches!(
        &preview.state.edit,
        Some(super::super::editing::EditDialog::Finished {
            outcome: EditSaveOutcome::Saved(_),
            ..
        })
    ));
    assert!(preview.state.mock_preview);
}

#[test]
fn cancelling_the_busy_mock_keeps_the_buffered_snapshot_and_never_opens_a_file() {
    let mut preview = Preview::new(Scene::Loading);
    preview.state.path = "/a/user/path/that/does/not/exist.accdb".into();
    let original_rows = preview
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert!(preview.state.busy);

    preview.dispatch(Action::Cancel);

    assert!(!preview.state.busy);
    assert_eq!(
        preview.state.error.as_deref(),
        Some("Mock operation cancelled."),
    );
    let retained_rows = preview
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(retained_rows, original_rows);
    assert_eq!(
        preview.state.database.as_ref().unwrap().document_revision,
        1
    );
    assert!(preview.state.query_data.is_none());
    assert!(preview.state.mock_preview);
}

use super::*;
use crate::{Action, EditSaveOutcome};

#[test]
fn every_existing_preview_scene_renders_without_a_runtime_or_file_host() {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut preview = Preview::new(Scene::Datasheet);
    for locale in crate::Locale::ALL {
        preview.state_mut().localizer = crate::Localizer::new(locale).unwrap();
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            preview.state_mut().theme = theme;
            for scene in Scene::ALL {
                preview.set_scene(scene);
                assert!(preview.state().mock_preview);
                for _ in 0..2 {
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1280.0, 820.0),
                            )),
                            ..Default::default()
                        },
                        |ui| preview.show(ui),
                    );
                    assert!(
                        !output.shapes.is_empty(),
                        "{locale}/{theme:?}/{} did not draw",
                        scene.as_str()
                    );
                    output.textures_delta.clear();
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

use super::*;
use crate::{Action, EditSaveOutcome};
use tablecraft_display::{CurrencyChoice, DateFormat, Options, TimeFormat};

mod declared_addition;
mod index_creation;
mod index_drop;

#[test]
fn every_existing_preview_scene_renders_without_a_runtime_or_file_host() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut preview = Preview::new(Scene::Datasheet);
    let formatting = Options {
        system_locale: Some("de-DE".into()),
        date: DateFormat::Iso,
        time: TimeFormat::HoursMinutesSeconds,
        currency: CurrencyChoice::Eur,
    };
    preview.state_mut().formatting = formatting.clone();
    for locale in crate::Locale::ALL {
        preview.state_mut().localizer = crate::Localizer::new(locale).unwrap();
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            preview.state_mut().theme = theme;
            for scene in Scene::ALL {
                preview.set_scene(scene);
                assert!(preview.state().mock_preview);
                assert_eq!(preview.state().formatting, formatting);
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

#[test]
fn objects_scene_retains_independent_tabs_without_running_saved_queries() {
    let mut preview = Preview::new(Scene::Objects);
    let tabs = preview.state.object_tabs();
    assert_eq!(tabs.len(), 4);
    let customers = tabs.iter().find(|tab| {
        matches!(&tab.target.kind, super::super::objects::ObjectKind::Table { name } if name == "Customers")
    }).unwrap();
    let saved = tabs
        .iter()
        .find(|tab| {
            matches!(
                tab.target.kind,
                super::super::objects::ObjectKind::SavedQuery { .. }
            )
        })
        .unwrap();
    assert_eq!(preview.state.active_object().as_ref(), Some(customers));
    preview.dispatch(crate::Action::ActivateObject { tab: saved.clone() });
    assert!(preview.state.saved_query.active);
    assert!(
        preview.state.saved_query.data.is_none(),
        "inspection is not execution"
    );
    preview.dispatch(crate::Action::ActivateObject {
        tab: customers.clone(),
    });
    assert_eq!(preview.state.data.as_ref().unwrap().table, "Customers");
    assert_eq!(preview.state.object_tabs().len(), 4);
}

#[test]
fn ribbon_display_scenes_keep_real_layout_states_and_the_same_loaded_records() {
    use egui::accesskit::Role;
    use egui_kittest::{Harness, kittest::Queryable};

    for scene in [Scene::RibbonCollapsed, Scene::RibbonTemporary] {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 820.0))
            .build_ui_state(
                |ui, preview: &mut Preview| preview.show(ui),
                Preview::new(scene),
            );
        crate::configure(&harness.ctx);
        harness.run_steps(3);
        let rows = harness.state().state().data.as_ref().unwrap().rows.as_ptr();
        let object = harness.state().state().active_object();
        assert!(harness.state().state().ribbon_collapsed());
        assert_eq!(
            harness.state().state().ribbon_temporarily_open(),
            scene == Scene::RibbonTemporary
        );
        harness.get_by_role_and_label(Role::Tab, "Home");
        assert_eq!(
            harness
                .query_by_role_and_label(Role::Button, "Copy")
                .is_some(),
            scene == Scene::RibbonTemporary
        );

        harness.state_mut().state_mut().set_ribbon_collapsed(false);
        harness.run_steps(3);
        assert!(!harness.state().state().ribbon_collapsed());
        assert!(!harness.state().state().ribbon_temporarily_open());
        harness.get_by_role_and_label(Role::Button, "Copy");
        assert_eq!(harness.state().state().active_object(), object);
        assert_eq!(
            harness.state().state().data.as_ref().unwrap().rows.as_ptr(),
            rows
        );
    }
}

#[test]
fn table_ready_previews_use_the_actual_table_command_and_loaded_name_suggestion() {
    use egui::accesskit::Role;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    for (scene, count, name) in [
        (Scene::FirstTableReady, 0, "Table1"),
        (Scene::AdditionalTableReady, 1, "Table2"),
    ] {
        assert_eq!(scene.as_str().parse::<Scene>().unwrap(), scene);
        let preview = Preview::new(scene);
        assert!(preview.state().mock_preview);
        assert!(preview.state().can_create_table());
        assert_eq!(
            preview.state().database.as_ref().unwrap().tables.len(),
            count
        );
        assert!(preview.state().data.is_none());
        assert!(preview.state().active_object().is_none());
        assert!(!preview.state().has_unsaved_table_drafts());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 820.0))
            .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
        crate::configure(&harness.ctx);
        harness.run_steps(2);
        let table = harness.get_by_role_and_label(Role::Button, "Table");
        assert!(!table.accesskit_node().is_disabled());
        table.click_accesskit();
        harness.run_steps(2);
        assert!(harness.state().state().has_unsaved_table_drafts());
        assert!(harness.state().state().data.is_none());
        assert_eq!(
            harness
                .state()
                .state()
                .database
                .as_ref()
                .unwrap()
                .tables
                .len(),
            count
        );
        harness.get_by_role_and_label(Role::Tab, name);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Click to Add")
                .is_focused()
        );
    }
}

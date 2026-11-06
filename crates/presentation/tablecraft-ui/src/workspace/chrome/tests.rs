use super::*;
use crate::Database;
use crate::Localizer;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

#[test]
fn long_workspace_title_retains_full_identity_without_displacing_status() {
    let context = egui::Context::default();
    crate::configure(&context);
    context.enable_accesskit();
    let title = "Quarterly international customer and supplier reconciliation workspace ".repeat(8);
    let mut state = State {
        database: Some(Database {
            title_path: None,
            document_revision: 7,
            title: title.clone(),
            format: "ACE16".into(),
            tables: vec![],
            queries: vec![],
        }),
        ..Default::default()
    };
    state.busy = true;
    let status = state.localizer.text("shell-working");
    for _ in 0..3 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 580.0),
                )),
                ..Default::default()
            },
            |ui| state.chrome(ui, &mut Vec::new()),
        );
        output.textures_delta.clear();
        let title_text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == title => {
                    Some((text, shape.clip_rect))
                }
                _ => None,
            })
            .expect("workspace title is painted");
        let status_text = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text)
                    if text.galley.job.text == status && text.pos.y < 60.0 =>
                {
                    Some((text, shape.clip_rect))
                }
                _ => None,
            })
            .expect("title-bar status is painted");
        let title_rect = egui::Rect::from_min_size(title_text.0.pos, title_text.0.galley.size());
        let status_rect = egui::Rect::from_min_size(status_text.0.pos, status_text.0.galley.size());
        assert!(
            title_text.0.galley.elided,
            "long title stays on one truncated line"
        );
        assert!(title_text.1.contains_rect(title_rect));
        assert!(status_text.1.contains_rect(status_rect));
        assert!(status_rect.right() <= 900.0);
        assert!(title_rect.right() <= status_rect.left());
        let tree = output.platform_output.accesskit_update.as_ref().unwrap();
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some(title.as_str())
                    || node.value() == Some(title.as_str()))
        );
    }
}

#[test]
fn short_workspace_title_keeps_the_same_left_edge_as_a_truncated_title() {
    let context = egui::Context::default();
    crate::configure(&context);
    let mut state = State::default();
    let mut starts = Vec::new();
    for title in ["Long workspace identity ".repeat(40), "Sample.accdb".into()] {
        state.database = Some(Database {
            title_path: None,
            document_revision: 7,
            title: title.clone(),
            format: "ACE16".into(),
            tables: vec![],
            queries: vec![],
        });
        for frame in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 580.0),
                    )),
                    ..Default::default()
                },
                |ui| state.chrome(ui, &mut Vec::new()),
            );
            output.textures_delta.clear();
            if frame == 2 {
                let x = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) if text.galley.job.text == title => {
                            Some(text.pos.x)
                        }
                        _ => None,
                    })
                    .expect("workspace title is painted");
                starts.push(x);
            }
        }
    }
    assert!(
        (starts[0] - starts[1]).abs() < 0.1,
        "short titles stay beside the application name"
    );
}

#[test]
fn quick_access_is_compact_and_inert_in_preview_and_real_workspace() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut preview = crate::preview::Preview::new(crate::preview::Scene::Datasheet);
    let mut quick_ids = Vec::new();
    for frame in 0..3 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 580.0),
                )),
                ..Default::default()
            },
            |ui| assert!(preview.state_mut().show(ui).is_empty()),
        );
        output.textures_delta.clear();
        if frame == 2 {
            let mut previous_right = None;
            for label in ["File", "Home", "Create", "External Data"] {
                let text = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text)
                            if text.galley.job.text == label && text.pos.y < 100.0 =>
                        {
                            Some(text)
                        }
                        _ => None,
                    })
                    .expect("actual Ribbon tab label is painted");
                if let Some(right) = previous_right {
                    assert!(
                        text.pos.x - right >= 16.0,
                        "adjacent painted labels retain their own side padding"
                    );
                }
                previous_right = Some(text.pos.x + text.galley.size().x);
            }
            let colors = palette(&context);
            let tray = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.corner_radius == egui::CornerRadius::same(5)
                            && rect.fill == colors.surface
                            && rect.rect.height() > 60.0 =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("rounded command tray is separate from the gray tab bar");
            assert!(
                tray.height() <= 100.0,
                "compact command groups do not gain excess height"
            );
            assert!(tray.left() >= 4.0 && tray.right() <= 896.0);
            let tree = output.platform_output.accesskit_update.as_ref().unwrap();
            let tab_top = tree
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.role() == egui::accesskit::Role::Tab && node.label() == Some("Home"))
                        .then(|| node.bounds())
                        .flatten()
                        .map(|rect| rect.y0)
                })
                .expect("active Ribbon tab");
            for label in ["Save", "Undo", "Redo"] {
                let (id, node) = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == egui::accesskit::Role::Button
                            && node.label() == Some(label)
                            && node.bounds().is_some_and(|rect| rect.y1 <= tab_top)
                    })
                    .expect("title-strip preview command");
                assert!(node.is_disabled());
                assert!(!node.supports_action(egui::accesskit::Action::Click));
                assert!(!node.supports_action(egui::accesskit::Action::Focus));
                let rect = node.bounds().unwrap();
                assert!(rect.x1 - rect.x0 <= 24.0);
                assert!(rect.y1 - rect.y0 <= 24.0);
                quick_ids.push(*id);
            }
        }
    }
    preview.state_mut().mock_preview = false;
    for id in quick_ids {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 580.0),
                )),
                events: vec![egui::Event::AccessKitActionRequest(
                    egui::accesskit::ActionRequest {
                        action: egui::accesskit::Action::Click,
                        target_tree: egui::accesskit::TreeId::ROOT,
                        target_node: id,
                        data: None,
                    },
                )],
                ..Default::default()
            },
            |ui| assert!(preview.state_mut().show(ui).is_empty()),
        );
        output.textures_delta.clear();
        let node = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find(|(actual, _)| *actual == id)
            .map(|(_, node)| node)
            .expect("real workspace retains the actual title command");
        assert!(node.is_disabled());
        assert!(!node.supports_action(egui::accesskit::Action::Click));
        assert!(!node.supports_action(egui::accesskit::Action::Focus));
    }
    assert!(!preview.state().edit_dialog_open());
    assert_eq!(preview.state().localizer.diagnostics(), Default::default());
}

#[test]
fn custom_window_controls_emit_real_viewport_requests_including_restore() {
    for (label, maximized, command) in [
        ("Minimize", false, egui::ViewportCommand::Minimized(true)),
        ("Maximize", false, egui::ViewportCommand::Maximized(true)),
        ("Restore", true, egui::ViewportCommand::Maximized(false)),
        ("Close window", false, egui::ViewportCommand::Close),
    ] {
        let context = egui::Context::default();
        context.enable_accesskit();
        crate::configure(&context);
        let localizer = Localizer::default();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 80.0),
            )),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(maximized);
        let mut output = context.run_ui(input.clone(), |ui| {
            ui.horizontal(|ui| window::show(ui, &localizer));
        });
        output.textures_delta.clear();
        let id = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find_map(|(id, node)| {
                (node.role() == egui::accesskit::Role::Button && node.label() == Some(label))
                    .then_some(*id)
            })
            .expect("actual window control");
        input.events.push(egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: id,
                data: None,
            },
        ));
        let mut output = context.run_ui(input, |ui| {
            ui.horizontal(|ui| window::show(ui, &localizer));
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&command)
        );
    }
}

#[test]
fn status_zoom_scales_the_datasheet_and_view_buttons_use_existing_actions() {
    let mut preview = crate::preview::Preview::new(crate::preview::Scene::Datasheet);
    preview.state_mut().theme = crate::Theme::Light;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, preview: &mut crate::preview::Preview| preview.show(ui),
            preview,
        );
    crate::configure(&harness.ctx);
    harness.run();
    let row_height = harness
        .get_all_by_role(egui::accesskit::Role::RowHeader)
        .next()
        .unwrap()
        .rect()
        .height();
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Zoom in")
        .click();
    harness.run();
    assert_eq!(crate::theme::datasheet_zoom(&harness.ctx), 110);
    assert_eq!(harness.ctx.zoom_factor(), 1.0);
    let zoomed_height = harness
        .get_all_by_role(egui::accesskit::Role::RowHeader)
        .next()
        .unwrap()
        .rect()
        .height();
    assert!(
        zoomed_height > row_height,
        "zoom changes actual datasheet metrics"
    );
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Design view")
        .click();
    harness.run();
    assert!(harness.state().state().view == DocumentView::Design);
    assert!(harness.state().state().schema.is_some());
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Button, "Zoom in")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Button, "Design view")
            .is_none()
    );
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Datasheet view")
        .click();
    harness.run();
    assert!(harness.state().state().view == DocumentView::Table);
    assert!(
        harness
            .state()
            .state()
            .database
            .as_ref()
            .unwrap()
            .title_path
            .is_some()
    );
    assert!(
        !harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Zoom in")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(
        harness.state().state().localizer.diagnostics(),
        Default::default()
    );
}

#[test]
fn quick_access_spacing_and_visibility_do_not_create_document_actions() {
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (Localizer, Vec<Action>)| {
            ui.horizontal(|ui| quick_access::show(ui, &state.0, true, &mut state.1));
        },
        (Localizer::default(), Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    let open = harness.get_by_role_and_label(egui::accesskit::Role::Button, "Open database file");
    let save = harness.get_by_role_and_label(egui::accesskit::Role::Button, "Save");
    assert!(save.rect().left() - open.rect().right() >= 6.0);
    assert!(save.accesskit_node().is_disabled());
    open.click_accesskit();
    harness.run_steps(1);
    assert!(matches!(harness.state().1.as_slice(), [Action::ChooseFile]));
    harness.state_mut().1.clear();
    harness
        .get_by_role_and_label(
            egui::accesskit::Role::Button,
            "Customize Quick Access Toolbar",
        )
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, "Save")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Button, "Save")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn footer_hides_unusable_sections_without_hiding_empty_grid_zoom() {
    use egui::accesskit::Role;

    let mut preview = crate::preview::Preview::new(crate::preview::Scene::Datasheet);
    let mut state = std::mem::take(preview.state_mut());
    state.data.as_mut().unwrap().rows.clear();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 80.0))
        .build_ui_state(
            |ui, state: &mut State| state.status_bar(ui, &mut Vec::new()),
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Slider, "Zoom")
            .is_some()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Design view")
            .is_some()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Datasheet view")
            .is_none()
    );

    harness.state_mut().show_open();
    harness.run();
    for (role, label) in [
        (Role::Slider, "Zoom"),
        (Role::Button, "Design view"),
        (Role::Button, "Datasheet view"),
    ] {
        assert!(harness.query_by_role_and_label(role, label).is_none());
    }
    harness.state_mut().dismiss_backstage();
    harness.state_mut().busy = true;
    harness.state_mut().data = None;
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Slider, "Zoom")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Design view")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Datasheet view")
            .is_none()
    );

    *harness.state_mut() = State::default();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Slider, "Zoom")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Design view")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Datasheet view")
            .is_none()
    );
}

#[test]
fn normal_workspace_tabs_retain_unavailable_commands_without_changing_object_or_dispatching() {
    let mut preview = crate::preview::Preview::new(crate::preview::Scene::Datasheet);
    let mut state = std::mem::take(preview.state_mut());
    state.mock_preview = false;
    let object = state.active_object().unwrap();
    let rows = state.data.as_ref().unwrap().rows.as_ptr();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    for (tab, unavailable) in [
        ("Create", "Query Wizard"),
        ("External Data", "New Data Source"),
        ("Database Tools", "Compact and Repair"),
        ("Help", "Help"),
        ("Table Fields", "New Field"),
        ("Table", "Before Change"),
    ] {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Tab, tab)
            .click_accesskit();
        harness.run_steps(3);
        let command = harness.get_by_role_and_label(egui::accesskit::Role::Button, unavailable);
        assert!(command.accesskit_node().is_disabled());
        assert!(
            !command
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        assert!(
            !command
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Focus)
        );
        assert_eq!(harness.state().0.active_object(), Some(object.clone()));
        assert_eq!(harness.state().0.data.as_ref().unwrap().rows.as_ptr(), rows);
        assert!(harness.state().1.is_empty());
    }
}

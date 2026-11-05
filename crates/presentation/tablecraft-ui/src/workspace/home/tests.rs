use super::*;
use crate::{
    Cell, Data,
    preview::{Preview, Scene},
};
use egui::{Event, FullOutput, PointerButton, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

fn render(
    state: &mut State,
    context: &egui::Context,
    events: Vec<Event>,
) -> (Vec<Action>, FullOutput) {
    let mut actions = Vec::new();
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 740.0),
            )),
            time: Some(context.cumulative_frame_nr() as f64 * 0.25),
            events,
            ..Default::default()
        },
        |ui| actions = state.show(ui),
    );
    output.textures_delta.clear();
    (actions, output)
}

fn click(state: &mut State, context: &egui::Context, label: &str) -> (Vec<Action>, FullOutput) {
    context.enable_accesskit();
    render(state, context, Vec::new());
    let (_, output) = render(state, context, Vec::new());
    let button_position = output
        .platform_output
        .accesskit_update
        .as_ref()
        .and_then(|tree| {
            tree.nodes.iter().find_map(|(_, node)| {
                if node.role() == egui::accesskit::Role::Button && node.label() == Some(label) {
                    node.bounds().map(|bounds| {
                        egui::pos2(
                            ((bounds.x0 + bounds.x1) * 0.5) as f32,
                            ((bounds.y0 + bounds.y1) * 0.5) as f32,
                        )
                    })
                } else {
                    None
                }
            })
        });
    let position = button_position
        .or_else(|| {
            output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
        })
        .expect("painted command or cell label");
    click_at(state, context, position)
}

fn click_at(
    state: &mut State,
    context: &egui::Context,
    position: egui::Pos2,
) -> (Vec<Action>, FullOutput) {
    render(
        state,
        context,
        vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    render(
        state,
        context,
        vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}

fn table() -> Data {
    Data {
        source_revision: Some(4),
        table: "Contacts".into(),
        offset: 100,
        columns: vec!["ID".into(), "Name".into()],
        rows: vec![vec![
            Cell::scalar("Old first cell"),
            Cell::scalar("Selected full cell text"),
        ]],
        has_more: false,
    }
}

#[test]
fn home_copy_uses_retained_cell_selection_after_toolbar_focus_changes() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.set_table_data(table());

    click(&mut state, &context, "Selected full cell text");
    assert_eq!(state.selected_cell_text(), Some("Selected full cell text"));
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(
                "Selected full cell text".into()
            ))
    );

    // A new page selects its new first cell, without retaining the old non-first
    // coordinate or requiring keyboard focus to copy the replacement value.
    let mut replacement = table();
    replacement.rows[0][0] = Cell::scalar("New first cell");
    state.set_table_data(replacement);
    let (_, output) = click(&mut state, &context, "Copy");
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("New first cell".into()))
    );
    assert_eq!(state.selected_cell_text(), Some("New first cell"));

    let mut empty = table();
    empty.rows.clear();
    state.set_table_data(empty);
    let (_, output) = click(&mut state, &context, "Copy");
    assert!(
        !output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
    );
}

#[test]
fn home_view_buttons_use_existing_table_navigation_and_schema_actions() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.set_table_data(table());
    state.select_view(DocumentView::Query);

    let (actions, _) = click(&mut state, &context, "Datasheet");
    assert!(actions.is_empty());
    assert!(state.view == DocumentView::Query);
    let (actions, _) = click(&mut state, &context, "Table Design");
    assert!(actions.is_empty());
    assert!(state.view == DocumentView::Query);

    state.select_view(DocumentView::Table);
    let (actions, _) = click(&mut state, &context, "Table Design");
    assert!(matches!(actions.as_slice(), [Action::ViewSchema { table }] if table == "Contacts"));

    state.prepare_schema_view();
    state.set_schema(crate::Schema {
        table: "Another table".into(),
        declared_rows: 0,
        fields: Vec::new(),
    });
    let (actions, _) = click(&mut state, &context, "Datasheet");
    assert!(actions.is_empty());
    assert!(state.view == DocumentView::Design);
    let (actions, _) = click(&mut state, &context, "Table Design");
    assert!(
        matches!(actions.as_slice(), [Action::ViewSchema { table }] if table == "Another table")
    );

    state.set_schema(crate::Schema {
        table: "Contacts".into(),
        declared_rows: 1,
        fields: Vec::new(),
    });
    let (actions, _) = click(&mut state, &context, "Datasheet");
    assert!(actions.is_empty());
    assert!(state.view == DocumentView::Table);

    state.busy = true;
    let (actions, _) = click(&mut state, &context, "Table Design");
    assert!(actions.is_empty());
    let (actions, _) = click(&mut state, &context, "File");
    assert!(actions.is_empty());
    assert!(state.backstage_visible());
    assert!(state.busy);
}

#[test]
fn row_selector_and_home_copy_share_complete_displayed_record_and_publication_resets_mode() {
    let context = egui::Context::default();
    let mut state = State::default();
    state.dismiss_backstage();
    state.database = Some(super::super::Database {
        document_revision: 4,
        title: "Example".into(),
        format: "ACE16".into(),
        tables: vec![],
        queries: vec![],
    });
    state.set_table_data(table());
    let target = state.data.as_ref().unwrap().edit_target(0, 0).unwrap();
    assert!(state.can_edit_cell(&target));
    context.enable_accesskit();
    let (_, output) = render(&mut state, &context, vec![]);
    let bounds = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual accessibility tree")
        .nodes
        .iter()
        .find_map(|(_, node)| {
            if node.role() == egui::accesskit::Role::RowHeader
                && node.label() == Some("Select record \u{2068}101\u{2069}")
            {
                node.bounds()
            } else {
                None
            }
        })
        .expect("actual record selector response bounds");
    let position = egui::pos2(
        ((bounds.x0 + bounds.x1) * 0.5) as f32,
        ((bounds.y0 + bounds.y1) * 0.5) as f32,
    );
    let (actions, _) = click_at(&mut state, &context, position);
    assert!(actions.is_empty());
    assert!(state.record_selected());
    assert!(state.selected_cell_text().is_none());
    assert!(!state.can_edit_cell(&target));
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText(
                "Old first cell\tSelected full cell text\n".into(),
            ))
    );
    let mut replacement = table();
    replacement.rows[0][0] = "NEW first".into();
    state.set_table_data(replacement);
    assert!(!state.record_selected());
    let (actions, output) = click(&mut state, &context, "Copy");
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("NEW first".into()))
    );
}

fn command_id(output: &FullOutput, label: &str) -> egui::accesskit::NodeId {
    output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual command accessibility tree")
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == egui::accesskit::Role::Button && node.label() == Some(label))
                .then_some(*id)
        })
        .expect("actual ribbon command")
}

fn assistive_action(target: egui::accesskit::NodeId, action: egui::accesskit::Action) -> Event {
    Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    })
}

#[test]
fn working_ribbon_commands_accept_keyboard_and_assistive_activation() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = State::default();
    state.dismiss_backstage();
    state.set_table_data(table());
    render(&mut state, &context, vec![]);
    let (_, output) = render(&mut state, &context, vec![]);
    let copy = command_id(&output, "Copy");

    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(copy, egui::accesskit::Action::Focus)],
    );
    assert!(actions.is_empty());
    assert_eq!(
        context
            .memory(|memory| memory.focused())
            .map(|id| id.accesskit_id()),
        Some(copy)
    );

    for key in [egui::Key::Enter, egui::Key::Space] {
        for pressed in [true, false] {
            let (actions, output) = render(
                &mut state,
                &context,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(actions.is_empty());
            let copies: Vec<_> = output
                .platform_output
                .commands
                .iter()
                .filter_map(|command| match command {
                    egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if pressed {
                assert_eq!(copies, ["Old first cell"]);
            } else {
                assert!(copies.is_empty());
            }
        }
    }

    let (actions, output) = render(
        &mut state,
        &context,
        vec![assistive_action(copy, egui::accesskit::Action::Click)],
    );
    assert!(actions.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .contains(&egui::OutputCommand::CopyText("Old first cell".into(),))
    );

    let design = command_id(&output, "Table Design");
    let (actions, _) = render(
        &mut state,
        &context,
        vec![assistive_action(design, egui::accesskit::Action::Click)],
    );
    assert!(matches!(actions.as_slice(), [Action::ViewSchema { table }] if table == "Contacts"));
}

#[test]
fn disabled_mock_commands_offer_no_assistive_actions_and_reject_forced_activation() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut state = State {
        mock_preview: true,
        ..Default::default()
    };
    state.dismiss_backstage();
    state.set_table_data(table());
    render(&mut state, &context, vec![]);
    let (_, output) = render(&mut state, &context, vec![]);
    let paste = command_id(&output, "Paste");
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    for label in ["Paste", "Create"] {
        let (_, node) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .unwrap();
        assert!(node.is_disabled());
        assert!(!node.supports_action(egui::accesskit::Action::Click));
        assert!(!node.supports_action(egui::accesskit::Action::Focus));
    }

    let (actions, output) = render(
        &mut state,
        &context,
        vec![
            assistive_action(paste, egui::accesskit::Action::Focus),
            assistive_action(paste, egui::accesskit::Action::Click),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(actions.is_empty());
    assert!(
        !output
            .platform_output
            .commands
            .iter()
            .any(|command| { matches!(command, egui::OutputCommand::CopyText(_)) })
    );
    assert_ne!(
        context
            .memory(|memory| memory.focused())
            .map(|id| id.accesskit_id()),
        Some(paste)
    );
    assert!(state.view == DocumentView::Table);
    assert_eq!(state.selected_cell_text(), Some("Old first cell"));
}

#[test]
fn mock_table_design_command_prepares_missing_schema_through_the_isolated_preview_host() {
    let preview = Preview::new(Scene::Datasheet);
    assert!(preview.state().schema.is_none());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run();

    let design = harness.get_by_role_and_label(Role::Button, "Table Design");
    assert!(!design.accesskit_node().is_disabled());
    design.click();
    harness.run();

    let state = harness.state().state();
    assert!(state.mock_preview);
    assert!(state.view == DocumentView::Design);
    let schema = state
        .schema
        .as_ref()
        .expect("the preview supplied its synthetic declaration");
    assert_eq!(schema.table, "Customers");
    assert!(!schema.fields.is_empty());
    assert_eq!(state.data.as_ref().unwrap().table, "Customers");
    assert_eq!(state.data.as_ref().unwrap().source_revision, Some(1));

    harness
        .get_by_role_and_label(Role::Button, "Datasheet")
        .click();
    harness.run();
    assert!(harness.state().state().view == DocumentView::Table);
    assert!(harness.state().state().mock_preview);

    harness.state_mut().state_mut().busy = true;
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Table Design")
            .accesskit_node()
            .is_disabled()
    );
}

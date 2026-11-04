use super::*;
use crate::{Cell, Data};
use egui::{Event, FullOutput, PointerButton};

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
    render(state, context, Vec::new());
    let (_, output) = render(state, context, Vec::new());
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("painted command or cell label");
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

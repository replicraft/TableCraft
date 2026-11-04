use super::*;
use egui::{Context, Event, OutputCommand, RawInput, Rect, Vec2};

fn input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(800.0, 400.0),
        )),
        events,
        ..Default::default()
    }
}

#[test]
fn ten_thousand_rows_only_draw_the_visible_subset() {
    let context = Context::default();
    let data = Data {
        table: "Large table".into(),
        offset: 100,
        columns: vec!["Value".into()],
        rows: (0..10_000).map(|row| vec![row.to_string()]).collect(),
        has_more: false,
    };
    for _ in 0..2 {
        let mut drawn = Vec::new();
        let mut output = context.run_ui(input(Vec::new()), |ui| {
            show_rows(ui, &data, |row| drawn.push(row));
        });
        output.textures_delta.clear();
        assert!(!drawn.is_empty());
        assert!(
            drawn.len() < 30,
            "offscreen rows were materialized: {}",
            drawn.len()
        );
        assert!(drawn.windows(2).all(|pair| pair[1] == pair[0] + 1));
    }
}

#[test]
fn focused_cell_copies_the_complete_value_and_exposes_row_and_column_context() {
    let context = Context::default();
    context.enable_accesskit();
    let value = "A long cell value that extends beyond the narrow visible column";
    let mut id = None;
    let mut initial = context.run_ui(input(Vec::new()), |ui| {
        ui.set_max_width(80.0);
        let response = cell(ui, value, "Description", 7);
        assert!(response.sense.is_focusable());
        id = Some(response.id);
        response.request_focus();
    });
    initial.textures_delta.clear();

    let mut output = context.run_ui(input(vec![Event::Copy]), |ui| {
        ui.set_max_width(80.0);
        let response = cell(ui, value, "Description", 7);
        assert_eq!(Some(response.id), id);
        assert!(response.has_focus());
    });
    output.textures_delta.clear();
    assert!(
        output
            .platform_output
            .commands
            .contains(&OutputCommand::CopyText(value.to_owned()))
    );
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accessibility update");
    assert!(tree.nodes.iter().any(|(_, node)| {
        node.value() == Some(format!("Description, row 7: {value}").as_str())
    }));
}

#[test]
fn tab_moves_between_cells_without_changing_their_text() {
    let context = Context::default();
    let mut first = None;
    let mut second = None;
    let mut initial = context.run_ui(input(Vec::new()), |ui| {
        first = Some(cell(ui, "First", "Name", 1).id);
        second = Some(cell(ui, "Second", "Name", 2).id);
    });
    initial.textures_delta.clear();

    context.memory_mut(|memory| memory.request_focus(first.unwrap()));
    let mut output = context.run_ui(
        input(vec![Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]),
        |ui| {
            assert_eq!(cell(ui, "First", "Name", 1).id, first.unwrap());
            let response = cell(ui, "Second", "Name", 2);
            assert_eq!(response.id, second.unwrap());
            assert!(response.has_focus());
        },
    );
    output.textures_delta.clear();
}

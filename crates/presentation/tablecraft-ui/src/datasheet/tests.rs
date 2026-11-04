use super::*;
use crate::CellKind;
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
        source_revision: None,
        table: "Large table".into(),
        offset: 100,
        columns: vec!["Value".into()],
        rows: (0..10_000)
            .map(|row| vec![Cell::scalar(row.to_string())])
            .collect(),
        has_more: false,
    };
    for _ in 0..2 {
        let mut drawn = Vec::new();
        let mut output = context.run_ui(input(Vec::new()), |ui| {
            show_rows(ui, &data, |row| drawn.push(row), &mut Vec::new());
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
    let value = Cell::scalar("A long cell value that extends beyond the narrow visible column");
    assert_focused_copy(
        &value,
        "Description, row 7: A long cell value that extends beyond the narrow visible column",
    );
}

#[test]
fn typed_cell_badges_do_not_change_copy_text_or_hide_accessible_provenance() {
    let cached_null = Cell {
        text: "NULL".into(),
        kind: CellKind::Scalar {
            is_null: true,
            stored_calculation: true,
        },
    };
    assert_focused_copy(
        &cached_null,
        "Description, row 7: NULL; Stored NULL calculated result; no formula evaluated; freshness unknown",
    );
    let attachment = Cell {
        text: "1 attachment".into(),
        kind: CellKind::Attachments {
            assigned: true,
            members: 1,
        },
    };
    assert_focused_copy(
        &attachment,
        "Description, row 7: 1 attachment; Assigned attachment collection; 1 complete member. NULL payloads count as members; payloads remain encoded and are not opened",
    );
}

fn assert_focused_copy(value: &Cell, accessible_value: &str) {
    let context = Context::default();
    context.enable_accesskit();
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
            .contains(&OutputCommand::CopyText(value.text.clone()))
    );
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accessibility update");
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.value() == Some(accessible_value))
    );
}

#[test]
fn tab_moves_between_cells_without_changing_their_text() {
    let context = Context::default();
    let mut first = None;
    let mut second = None;
    let mut initial = context.run_ui(input(Vec::new()), |ui| {
        first = Some(cell(ui, &Cell::scalar("First"), "Name", 1).id);
        second = Some(cell(ui, &Cell::scalar("Second"), "Name", 2).id);
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
            assert_eq!(
                cell(ui, &Cell::scalar("First"), "Name", 1).id,
                first.unwrap()
            );
            let response = cell(ui, &Cell::scalar("Second"), "Name", 2);
            assert_eq!(response.id, second.unwrap());
            assert!(response.has_focus());
        },
    );
    output.textures_delta.clear();
}

#[test]
fn narrow_cached_result_cell_paints_the_value_before_its_annotation() {
    let context = Context::default();
    let value = Cell {
        text: "77".into(),
        kind: CellKind::Scalar {
            is_null: false,
            stored_calculation: true,
        },
    };
    let mut output = context.run_ui(input(Vec::new()), |ui| {
        ui.set_max_width(128.0);
        cell(ui, &value, "Calculated", 1);
    });
    assert!(output.shapes.iter().any(|shape| {
        if let egui::epaint::Shape::Text(text) = &shape.shape {
            text.galley.job.text.starts_with("77")
        } else {
            false
        }
    }));
    output.textures_delta.clear();
}

#[derive(Clone, Copy)]
struct ScrollObservation {
    offset: egui::Vec2,
    viewport: Rect,
    content_size: egui::Vec2,
    bar: Option<Rect>,
}

fn scroll_frame(context: &Context, data: &Data, events: Vec<Event>) -> ScrollObservation {
    let mut observed = None;
    let mut frame_input = input(events);
    // Advance a deterministic clock past visibility animations between frames.
    frame_input.time = Some(context.cumulative_frame_nr() as f64 * 0.25);
    let mut output = context.run_ui(frame_input, |ui| {
        let area = show_rows(ui, data, |_| {}, &mut Vec::new());
        let bar = context.read_response(area.id.with(0_usize));
        observed = Some(ScrollObservation {
            offset: area.state.offset,
            viewport: area.inner_rect,
            content_size: area.content_size,
            bar: bar.map(|response| response.rect),
        });
    });
    output.textures_delta.clear();
    observed.unwrap()
}

fn wheel(delta: f32, pointer: egui::Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(pointer),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(delta, 0.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn horizontal_wheel_and_scrollbar_drag_move_actual_one_and_hundred_row_tables() {
    for rows in [1, 100] {
        let context = Context::default();
        let data = Data {
            source_revision: None,
            table: format!("Wide table with {rows} rows"),
            offset: 0,
            columns: (0..16).map(|column| format!("Column {column}")).collect(),
            rows: (0..rows)
                .map(|row| {
                    (0..16)
                        .map(|column| Cell::scalar(format!("{row}:{column}")))
                        .collect()
                })
                .collect(),
            has_more: false,
        };
        // Size passes and scrollbar visibility settle before pointer hit-testing.
        for _ in 0..2 {
            scroll_frame(&context, &data, Vec::new());
        }
        let before = scroll_frame(&context, &data, Vec::new());
        assert!(before.content_size.x > before.viewport.width());
        assert!(
            before.viewport.height() > 300.0,
            "one-row page collapsed its viewport"
        );
        assert!(
            before.bar.expect("settled horizontal scrollbar").top() >= before.viewport.bottom(),
            "scrollbar overlays data: viewport={:?}, bar={:?}, content={:?}",
            before.viewport,
            before.bar,
            before.content_size,
        );
        let right = scroll_frame(&context, &data, wheel(-120.0, before.viewport.center()));
        assert!(
            right.offset.x > before.offset.x,
            "horizontal wheel did not move {rows}-row table"
        );
        assert_eq!(right.offset.y, 0.0);
        let left = scroll_frame(&context, &data, wheel(120.0, before.viewport.center()));
        assert!(
            left.offset.x < right.offset.x,
            "reverse wheel did not move {rows}-row table"
        );

        // Use actual logical scrollbar geometry, never screenshot/Retina coordinates.
        let bar = left.bar.expect("horizontal scrollbar");
        let start = egui::pos2(bar.left() + 12.0, bar.center().y);
        scroll_frame(&context, &data, vec![Event::PointerMoved(start)]);
        scroll_frame(
            &context,
            &data,
            vec![Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let target = start + egui::vec2(140.0, 0.0);
        let dragged = scroll_frame(&context, &data, vec![Event::PointerMoved(target)]);
        let released = scroll_frame(
            &context,
            &data,
            vec![Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            dragged.offset.x > left.offset.x,
            "scrollbar drag did not move {rows}-row table"
        );
        assert!(released.offset.x > left.offset.x);
    }
}

#[test]
fn focused_native_collection_enter_emits_exact_target_but_copy_and_modified_enter_do_not() {
    let context = Context::default();
    let mut data = Data {
        source_revision: Some(9),
        table: "Parents".into(),
        offset: 100,
        columns: vec!["Items".into()],
        rows: vec![vec![Cell {
            text: "2 items".into(),
            kind: CellKind::Multivalue {
                assigned: true,
                members: 2,
            },
        }]],
        has_more: false,
    };
    let mut first = context.run_ui(input(Vec::new()), |ui| {
        cell(ui, &data.rows[0][0], "Items", 101).request_focus();
    });
    first.textures_delta.clear();
    for (modifiers, expected) in [
        (egui::Modifiers::NONE, true),
        (
            egui::Modifiers {
                shift: true,
                ..Default::default()
            },
            false,
        ),
        (
            egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
            false,
        ),
    ] {
        let mut output = context.run_ui(
            input(vec![Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }]),
            |ui| {
                let response = cell(ui, &data.rows[0][0], "Items", 101);
                let mut actions = Vec::new();
                collect_contents_action(ui, &response, &data, (0, 0), &mut actions);
                if expected {
                    assert!(
                        matches!(actions.as_slice(), [Action::ViewContents { target }]
                    if target.document_revision == 9 && target.table == "Parents"
                    && target.column == "Items" && target.parent_ordinal == 100)
                    );
                } else {
                    assert!(actions.is_empty());
                }
            },
        );
        output.textures_delta.clear();
    }
    let mut output = context.run_ui(input(vec![Event::Copy]), |ui| {
        let response = cell(ui, &data.rows[0][0], "Items", 101);
        let mut actions = Vec::new();
        collect_contents_action(ui, &response, &data, (0, 0), &mut actions);
        assert!(actions.is_empty());
    });
    assert!(
        output
            .platform_output
            .commands
            .contains(&OutputCommand::CopyText("2 items".into()))
    );
    output.textures_delta.clear();

    data.source_revision = None;
    let mut output = context.run_ui(
        input(vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]),
        |ui| {
            let response = cell(ui, &data.rows[0][0], "Items", 101);
            let mut actions = Vec::new();
            collect_contents_action(ui, &response, &data, (0, 0), &mut actions);
            assert!(actions.is_empty());
        },
    );
    output.textures_delta.clear();
}

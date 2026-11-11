use super::*;
use crate::CellKind;
use egui::{
    Context, Event, OutputCommand, RawInput, Rect, Vec2,
    accesskit::{Action as AccessibleAction, Role, Toggled},
};

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
        query_contents: None,
        row_ordinals: None,
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
            show_rows(
                ui,
                &data,
                |row| drawn.push(row),
                &mut Vec::new(),
                &mut Selection::default(),
                Pane::Document,
            );
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
        presentation: None,
        alignment: crate::HorizontalAlignment::Left,
        text: "NULL".into(),
        kind: CellKind::Scalar {
            is_null: true,
            stored_calculation: true,
            boolean: None,
            display_control: crate::DisplayControl::Uninspected,
        },
    };
    assert_focused_copy(
        &cached_null,
        "Description, row 7: NULL; Stored NULL calculated result; no formula evaluated; freshness unknown",
    );
    let attachment = Cell {
        presentation: None,
        alignment: crate::HorizontalAlignment::Left,
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

#[test]
fn german_typed_summary_paints_localized_text_but_copies_the_original_host_value() {
    let context = Context::default();
    let localizer = crate::Localizer::new(crate::Locale::De).unwrap();
    crate::localization::install(&context, &localizer);
    let value = Cell {
        presentation: None,
        alignment: crate::HorizontalAlignment::Left,
        text: "2 attachments".into(),
        kind: CellKind::Attachments {
            assigned: true,
            members: 2,
        },
    };
    let label = value.state_label(&localizer).unwrap();
    let _ = context.run_ui(input(vec![]), |ui| {
        cell(ui, &value, "Files", 1, true).request_focus();
    });
    let output = context.run_ui(input(vec![Event::Copy]), |ui| {
        cell(ui, &value, "Files", 1, true);
    });
    assert!(output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text)
            if text.galley.job.text.contains(&label))
    }));
    assert!(
        output
            .platform_output
            .commands
            .contains(&OutputCommand::CopyText("2 attachments".into()))
    );
    assert_eq!(value.text, "2 attachments");
}

#[test]
fn large_cell_preview_bounds_paint_and_accessibility_while_copy_preserves_complete_text() {
    for (text, prefix) in [
        ("a".repeat(100_000) + "tail", "a".repeat(512) + "…"),
        ("界".repeat(100_000) + "tail", "界".repeat(341) + "…"),
        ("\n".repeat(100_000) + "tail", "\n".repeat(7) + "…"),
    ] {
        let value = Cell {
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: true,
                boolean: None,
                display_control: crate::DisplayControl::Uninspected,
            },
            ..Cell::scalar(text)
        };
        let expected = format!(
            "Description, row 7: {prefix}; Stored calculated result; no formula evaluated; freshness unknown Preview shortened. Copy preserves the complete displayed value."
        );
        assert_focused_copy(&value, &expected);
    }
}

fn assert_focused_copy(value: &Cell, accessible_value: &str) {
    let context = Context::default();
    context.enable_accesskit();
    let mut id = None;
    let mut initial = context.run_ui(input(Vec::new()), |ui| {
        ui.set_max_width(80.0);
        let response = cell(ui, value, "Description", 7, true);
        assert!(response.sense.is_focusable());
        id = Some(response.id);
        response.request_focus();
    });
    initial.textures_delta.clear();

    // A newly created popup measures itself in an invisible first frame.
    // Settle that frame before observing focused-cell painting and Copy.
    let mut settled = context.run_ui(input(vec![Event::PointerGone]), |ui| {
        ui.set_max_width(80.0);
        let response = cell(ui, value, "Description", 7, true);
        assert!(response.has_focus());
    });
    settled.textures_delta.clear();

    let mut output = context.run_ui(input(vec![Event::PointerGone, Event::Copy]), |ui| {
        ui.set_max_width(80.0);
        let response = cell(ui, value, "Description", 7, true);
        assert_eq!(Some(response.id), id);
        assert!(response.has_focus());
    });
    output.textures_delta.clear();
    let copy_help = crate::localization::for_context(&context).text("datasheet-copy-cell-help");
    for shape in &output.shapes {
        if let egui::Shape::Text(painted) = &shape.shape {
            assert!(painted.galley.job.text.len() < 4096);
            assert_ne!(
                painted.galley.job.text, copy_help,
                "keyboard focus without a pointer must not paint a cell tooltip",
            );
        }
    }
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
    for (_, node) in &tree.nodes {
        for text in [node.value(), node.label(), node.description()]
            .into_iter()
            .flatten()
        {
            assert!(text.len() < 4096);
        }
    }
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.value().is_some_and(|value| {
                value.replace(['\u{2068}', '\u{2069}'], "") == accessible_value
            }))
    );
}

#[test]
fn tab_moves_between_cells_without_changing_their_text() {
    let context = Context::default();
    let mut first = None;
    let mut second = None;
    let mut initial = context.run_ui(input(Vec::new()), |ui| {
        first = Some(cell(ui, &Cell::scalar("First"), "Name", 1, true).id);
        second = Some(cell(ui, &Cell::scalar("Second"), "Name", 2, true).id);
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
                cell(ui, &Cell::scalar("First"), "Name", 1, true).id,
                first.unwrap()
            );
            let response = cell(ui, &Cell::scalar("Second"), "Name", 2, true);
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
        presentation: None,
        alignment: crate::HorizontalAlignment::Left,
        text: "77".into(),
        kind: CellKind::Scalar {
            is_null: false,
            stored_calculation: true,
            boolean: None,
            display_control: crate::DisplayControl::Uninspected,
        },
    };
    let mut output = context.run_ui(input(Vec::new()), |ui| {
        ui.set_max_width(128.0);
        cell(ui, &value, "Calculated", 1, true);
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
        let area = show_rows(
            ui,
            data,
            |_| {},
            &mut Vec::new(),
            &mut Selection::default(),
            Pane::Document,
        );
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
            query_contents: None,
            row_ordinals: None,
            source_revision: None,
            table: format!("Wide table with {rows} rows"),
            offset: 0,
            columns: (0..16)
                .map(|column| format!("Column {column}").into())
                .collect(),
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
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(9),
        table: "Parents".into(),
        offset: 100,
        columns: vec!["Items".into()],
        rows: vec![vec![Cell {
            presentation: None,
            alignment: crate::HorizontalAlignment::Left,
            text: "2 items".into(),
            kind: CellKind::Multivalue {
                assigned: true,
                members: 2,
            },
        }]],
        has_more: false,
    };
    let mut first = context.run_ui(input(Vec::new()), |ui| {
        cell(ui, &data.rows[0][0], "Items", 101, true).request_focus();
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
                let response = cell(ui, &data.rows[0][0], "Items", 101, true);
                let mut actions = Vec::new();
                collect_contents_action(ui, &response, &data, (0, 0), false, false, &mut actions);
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
        let response = cell(ui, &data.rows[0][0], "Items", 101, true);
        let mut actions = Vec::new();
        collect_contents_action(ui, &response, &data, (0, 0), false, false, &mut actions);
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
            let response = cell(ui, &data.rows[0][0], "Items", 101, true);
            let mut actions = Vec::new();
            collect_contents_action(ui, &response, &data, (0, 0), false, false, &mut actions);
            assert!(actions.is_empty());
        },
    );
    output.textures_delta.clear();
}

fn selection_frame(
    context: &Context,
    data: &Data,
    selection: &mut Selection,
    events: Vec<Event>,
) -> (egui::FullOutput, Vec<usize>, egui::Vec2) {
    let mut drawn = Vec::new();
    let mut offset = egui::Vec2::ZERO;
    let mut frame_input = input(events);
    frame_input.time = Some(context.cumulative_frame_nr() as f64 * 0.25);
    let output = context.run_ui(frame_input, |ui| {
        offset = show_rows(
            ui,
            data,
            |row| drawn.push(row),
            &mut Vec::new(),
            selection,
            Pane::Document,
        )
        .state
        .offset;
    });
    (output, drawn, offset)
}

#[test]
fn full_cell_click_and_arrows_select_loaded_cells_and_reveal_offscreen_columns_once() {
    let context = Context::default();
    let data = Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(7),
        table: "Wide selected table".into(),
        offset: 100,
        columns: (0..16)
            .map(|column| format!("Field {column}").into())
            .collect(),
        rows: (0..100)
            .map(|row| {
                (0..16)
                    .map(|column| Cell::scalar(format!("{row}:{column}")))
                    .collect()
            })
            .collect(),
        has_more: true,
    };
    let mut selection = Selection::default();
    click_grid_cell(
        &context,
        &data,
        &mut selection,
        (0, 1),
        egui::PointerButton::Primary,
    );
    assert_eq!(selection.text(&data), Some("0:1"));
    // Each native-like keydown and keyup is its own frame. No idle frame
    // is inserted to mask egui's pre-UI cardinal focus navigation.
    let mut expected_column = 1;
    for key in
        std::iter::once(egui::Key::ArrowDown).chain(std::iter::repeat_n(egui::Key::ArrowRight, 14))
    {
        if key == egui::Key::ArrowRight {
            expected_column += 1;
        }
        for pressed in [true, false] {
            let (mut output, _, _) = selection_frame(
                &context,
                &data,
                &mut selection,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            output.textures_delta.clear();
            assert_eq!(
                selection.text(&data),
                Some(format!("1:{expected_column}").as_str())
            );
            assert_eq!(
                context.memory(|memory| memory.focused()),
                selection.focus_id(),
                "grid focus escaped after {key:?}, pressed={pressed}"
            );
        }
    }
    let (mut output, drawn, offset) = selection_frame(&context, &data, &mut selection, vec![]);
    assert_eq!(selection.text(&data), Some("1:15"));
    assert!(drawn.len() < 30);
    assert!(offset.x > 0.0, "rightmost selected column was not revealed");
    output.textures_delta.clear();
    // Default programmatic scrolling animates for up to 300 ms. Observe a
    // settled baseline before measuring an independent manual wheel gesture.
    let (mut output, _, prior) = selection_frame(&context, &data, &mut selection, vec![]);
    output.textures_delta.clear();
    let (mut output, _, offset) = selection_frame(&context, &data, &mut selection, vec![]);
    output.textures_delta.clear();
    assert!(
        (offset.x - prior.x).abs() < 0.01,
        "programmatic reveal did not settle"
    );
    let (mut output, _, moved) = selection_frame(
        &context,
        &data,
        &mut selection,
        wheel(120.0, egui::pos2(400.0, 150.0)),
    );
    output.textures_delta.clear();
    let (mut output, _, settled) = selection_frame(&context, &data, &mut selection, vec![]);
    assert!(
        moved.x < offset.x,
        "manual scroll was pinned to selected cell"
    );
    assert!(settled.x <= offset.x);
    assert_eq!(selection.text(&data), Some("1:15"));
    output.textures_delta.clear();
}

fn click_grid_cell(
    context: &Context,
    data: &Data,
    selection: &mut Selection,
    position: (usize, usize),
    button: egui::PointerButton,
) {
    context.enable_accesskit();
    let (row, column) = position;
    let name = format!(
        "\u{2068}{}\u{2069}, row \u{2068}{}\u{2069}: \u{2068}{}\u{2069}",
        data.columns[column].name,
        u64::from(data.offset) + row as u64 + 1,
        data.rows[row][column].text
    );
    let mut cell_rect = None;
    let mut painted_right = None;
    let mut painted_left = None;
    for _ in 0..3 {
        let (mut output, _, _) = selection_frame(context, data, selection, vec![]);
        if let Some(tree) = &output.platform_output.accesskit_update {
            for (_, node) in &tree.nodes {
                if node.value().is_some_and(|value| value.starts_with(&name))
                    && let Some(bounds) = node.bounds()
                {
                    cell_rect = Some(Rect::from_min_max(
                        egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
                        egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
                    ));
                }
            }
        }
        for shape in &output.shapes {
            if let egui::epaint::Shape::Text(text) = &shape.shape
                && text
                    .galley
                    .job
                    .text
                    .starts_with(&data.rows[row][column].text)
                && cell_rect.is_some_and(|rect| rect.contains(text.pos))
            {
                painted_left = Some(text.pos.x);
                painted_right = Some(text.pos.x + text.galley.size().x);
            }
        }
        output.textures_delta.clear();
    }
    let rect = cell_rect.expect("actual accessible cell response bounds");
    let left = painted_left.expect("cell value painted");
    assert!(
        (left - rect.left() - 3.0).abs() < 0.5,
        "text is not at the cell's left inset: text={left}, cell={rect:?}"
    );
    // The table's full-height resize handle overlaps the column boundary.
    // With no intercell gutter, click beyond its grab radius while retaining
    // the trailing blank-cell-area check below.
    let resize_grab_radius = context
        .style_of(context.theme())
        .interaction
        .resize_grab_radius_side;
    let pointer = egui::pos2(rect.right() - resize_grab_radius - 2.0, rect.center().y);
    assert!(rect.contains(pointer));
    if data.rows[row][column].text == "0:1" {
        assert!(
            pointer.x > painted_right.expect("short cell value painted"),
            "click must exercise blank trailing cell area"
        );
    }
    for pressed in [true, false] {
        let (mut output, _, _) = selection_frame(
            context,
            data,
            selection,
            vec![
                Event::PointerMoved(pointer),
                Event::PointerButton {
                    pos: pointer,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output.textures_delta.clear();
    }
}

#[test]
fn same_identity_publication_surrenders_old_grid_focus_before_contents_enter() {
    let context = Context::default();
    let mut data = Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(9),
        table: "Parents".into(),
        offset: 0,
        columns: vec!["First".into(), "Items".into()],
        rows: vec![vec![Cell::scalar("old first"), Cell::scalar("old second")]],
        has_more: false,
    };
    let mut selection = Selection::default();
    click_grid_cell(
        &context,
        &data,
        &mut selection,
        (0, 1),
        egui::PointerButton::Primary,
    );
    assert_eq!(selection.text(&data), Some("old second"));
    data.rows[0] = vec![
        Cell::scalar("NEW"),
        Cell {
            presentation: None,
            alignment: crate::HorizontalAlignment::Left,
            text: "1 item".into(),
            kind: CellKind::Multivalue {
                assigned: true,
                members: 1,
            },
        },
    ];
    selection.reset(&data);
    let mut output = context.run_ui(
        input(vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]),
        |ui| {
            let mut actions = Vec::new();
            show_rows(
                ui,
                &data,
                |_| {},
                &mut actions,
                &mut selection,
                Pane::Document,
            );
            assert!(
                actions.is_empty(),
                "old focused column opened new collection"
            );
            assert!(ui.memory(|memory| memory.focused()).is_none());
        },
    );
    output.textures_delta.clear();
    assert_eq!(selection.text(&data), Some("NEW"));
    let (mut output, _, _) = selection_frame(
        &context,
        &data,
        &mut selection,
        vec![Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    output.textures_delta.clear();
    assert_eq!(selection.text(&data), Some("NEW"));

    // Right-clicking another cell changes the retained cell before its menu.
    click_grid_cell(
        &context,
        &data,
        &mut selection,
        (0, 1),
        egui::PointerButton::Secondary,
    );
    assert_eq!(selection.text(&data), Some("1 item"));
}

#[test]
fn typed_boolean_checkbox_is_accessibly_read_only_and_copies_original_text() {
    for checked in [false, true] {
        let context = Context::default();
        context.enable_accesskit();
        let value = Cell {
            presentation: None,
            alignment: crate::HorizontalAlignment::Left,
            text: if checked { "-1".into() } else { "0".into() },
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: true,
                boolean: Some(checked),
                display_control: crate::DisplayControl::Native(106),
            },
        };
        let mut bounds = None;
        let mut first = context.run_ui(input(vec![]), |ui| {
            ui.set_max_width(128.0);
            let response = cell(ui, &value, "YesNo", 1, true);
            bounds = Some(response.rect);
            response.request_focus();
        });
        let cell_bounds = bounds.unwrap();
        let checkbox = first
            .shapes
            .iter()
            .find_map(|shape| {
                let egui::Shape::Rect(rectangle) = &shape.shape else {
                    return None;
                };
                (rectangle.stroke.color == crate::theme::palette(&context).checkbox_accent
                    && rectangle.rect.width() == BOOLEAN_BOX_SIZE)
                    .then_some(rectangle.rect)
            })
            .expect("actual readonly checkbox artwork");
        assert_eq!(checkbox.center(), cell_bounds.center());
        first.textures_delta.clear();
        let pointer = bounds.unwrap().center();
        for events in [
            vec![
                Event::PointerMoved(pointer),
                Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![
                Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                Event::Copy,
            ],
        ] {
            let copying = events.iter().any(|event| matches!(event, Event::Copy));
            let mut output = context.run_ui(input(events), |ui| {
                ui.set_max_width(128.0);
                let response = cell(ui, &value, "YesNo", 1, true);
                assert!(!response.changed());
                assert!(response.sense.is_focusable());
            });
            let tree = output
                .platform_output
                .accesskit_update
                .as_ref()
                .expect("accessibility update");
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == Role::CheckBox)
                .expect("typed checkbox node");
            assert_eq!(
                node.1.toggled(),
                Some(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
            );
            assert!(node.1.is_read_only());
            assert!(!node.1.supports_action(AccessibleAction::Click));
            assert!(node.1.label().unwrap().contains("freshness unknown"));
            if copying {
                assert!(
                    output
                        .platform_output
                        .commands
                        .contains(&OutputCommand::CopyText(value.text.clone()))
                );
            }
            assert_eq!(value.checkbox_value(), Some(checked));
            output.textures_delta.clear();
        }
    }
}

#[test]
fn shift_space_selects_complete_record_copy_and_returns_to_first_field_without_opening_children() {
    let context = Context::default();
    context.enable_accesskit();
    let data = Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(7),
        table: "Parents".into(),
        offset: 0,
        columns: vec!["ID".into(), "Nullable".into(), "Items".into()],
        rows: vec![
            vec![
                "0:0".into(),
                Cell {
                    presentation: None,
                    alignment: crate::HorizontalAlignment::Left,
                    text: "NULL".into(),
                    kind: CellKind::Scalar {
                        is_null: true,
                        stored_calculation: false,
                        boolean: None,
                        display_control: crate::DisplayControl::Uninspected,
                    },
                },
                Cell {
                    presentation: None,
                    alignment: crate::HorizontalAlignment::Left,
                    text: "2 items".into(),
                    kind: CellKind::Multivalue {
                        assigned: true,
                        members: 2,
                    },
                },
            ],
            vec!["1:0".into(), "1:1".into(), "1:2".into()],
        ],
        has_more: false,
    };
    let mut selection = Selection::default();
    selection.reset(&data);
    let (mut output, _, _) = selection_frame(&context, &data, &mut selection, vec![]);
    for shape in &output.shapes {
        if let egui::epaint::Shape::Text(text) = &shape.shape {
            assert!(!["#", "1", "2"].contains(&text.galley.job.text.as_str()));
        }
    }
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual selector accessibility tree");
    for label in [
        "Select record \u{2068}1\u{2069}",
        "Select record \u{2068}2\u{2069}",
    ] {
        let selector = tree
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.role() == Role::RowHeader && node.label() == Some(label)).then_some(node)
            })
            .expect("accessible selector retains its record number");
        let bounds = selector.bounds().expect("actual selector hit target");
        let width = bounds.x1 - bounds.x0;
        // The retained capture's selector strip is close to one row's height.
        assert!((14.0..=22.0).contains(&width), "selector width: {width}");
    }
    output.textures_delta.clear();

    click_grid_cell(
        &context,
        &data,
        &mut selection,
        (0, 2),
        egui::PointerButton::Primary,
    );
    for pressed in [true, false] {
        let mut events = vec![record_key(
            egui::Key::Space,
            pressed,
            egui::Modifiers::SHIFT,
        )];
        if pressed {
            // The toolkit can deliver the shortcut and Copy in one frame.
            events.push(Event::Copy);
        }
        let (mut output, _, _) = selection_frame(&context, &data, &mut selection, events);
        if pressed {
            let record_copies = output.platform_output.commands.iter().filter(|command| {
                matches!(command, OutputCommand::CopyText(text) if text == "0:0\tNULL\t2 items\n")
            }).count();
            assert_eq!(record_copies, 1);
        }
        output.textures_delta.clear();
    }
    assert!(selection.row_selected());
    assert_eq!(
        context.memory(|memory| memory.focused()),
        selection.focus_id()
    );
    assert_eq!(selection.position(), Some((0, 2)));
    assert!(selection.text(&data).is_none());
    let (mut output, _, _) = selection_frame(&context, &data, &mut selection, vec![Event::Copy]);
    let copied: Vec<_> = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| {
            if let OutputCommand::CopyText(text) = command {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(copied, ["0:0\tNULL\t2 items\n"]);
    output.textures_delta.clear();

    let mut output = context.run_ui(
        input(vec![record_key(
            egui::Key::Enter,
            true,
            egui::Modifiers::NONE,
        )]),
        |ui| {
            assert!(
                show(
                    ui,
                    &data,
                    &mut selection,
                    Pane::Document,
                    Features::default()
                )
                .actions
                .is_empty()
            );
        },
    );
    output.textures_delta.clear();
    assert!(selection.row_selected());
    for pressed in [true, false] {
        let (mut output, _, _) = selection_frame(
            &context,
            &data,
            &mut selection,
            vec![record_key(
                egui::Key::Space,
                pressed,
                egui::Modifiers::SHIFT,
            )],
        );
        output.textures_delta.clear();
    }
    assert!(!selection.row_selected());
    assert_eq!(selection.position(), Some((0, 0)));

    for (key, modifiers) in [
        (egui::Key::Space, egui::Modifiers::SHIFT),
        (egui::Key::ArrowDown, egui::Modifiers::NONE),
    ] {
        for pressed in [true, false] {
            let (mut output, _, _) = selection_frame(
                &context,
                &data,
                &mut selection,
                vec![record_key(key, pressed, modifiers)],
            );
            output.textures_delta.clear();
        }
    }
    assert!(!selection.row_selected());
    assert_eq!(selection.text(&data), Some("1:0"));

    let mut editor = String::new();
    let mut output = context.run_ui(input(vec![]), |ui| {
        ui.add(egui::TextEdit::singleline(&mut editor).id(egui::Id::new("space_editor")))
            .request_focus();
        let _ = show(
            ui,
            &data,
            &mut selection,
            Pane::Document,
            Features::default(),
        );
    });
    output.textures_delta.clear();
    let mut output = context.run_ui(
        input(vec![
            record_key(egui::Key::Space, true, egui::Modifiers::SHIFT),
            Event::Text(" ".into()),
        ]),
        |ui| {
            ui.add(egui::TextEdit::singleline(&mut editor).id(egui::Id::new("space_editor")));
            let _ = show(
                ui,
                &data,
                &mut selection,
                Pane::Document,
                Features::default(),
            );
        },
    );
    output.textures_delta.clear();
    assert_eq!(editor, " ");
    assert!(!selection.row_selected());
}

fn record_key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn explicit_alignment_changes_paint_position_without_inference_or_copy_changes() {
    let context = Context::default();
    let left = Cell::scalar("123.00");
    let mut right = Cell::scalar("123.00");
    right.alignment = crate::HorizontalAlignment::Right;
    let mut bounds = Vec::new();
    let mut output = context.run_ui(input(vec![]), |ui| {
        for value in [&left, &right] {
            ui.allocate_ui_with_layout(
                egui::vec2(240.0, 24.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    bounds.push(cell(ui, value, "Value", 1, false).rect);
                },
            );
        }
    });
    output.textures_delta.clear();
    let painted: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "123.00" => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(painted.len(), 2);
    assert!((painted[0].pos.x - bounds[0].left() - CELL_TEXT_PADDING).abs() < 0.1);
    assert!(
        (painted[1].pos.x + painted[1].galley.size().x - bounds[1].right() + CELL_TEXT_PADDING)
            .abs()
            < 0.1
    );
    assert_eq!(left.alignment, crate::HorizontalAlignment::Left);
    assert_eq!(left.text, right.text);
    assert_focused_copy(&right, "Description, row 7: 123.00");
}

#[test]
fn typed_null_is_visible_but_empty_text_stays_empty_and_headers_are_centered() {
    let context = Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut null = Cell::scalar("");
    null.kind = CellKind::Scalar {
        is_null: true,
        stored_calculation: false,
        boolean: None,
        display_control: crate::DisplayControl::Absent,
    };
    let data = Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(7),
        table: "Null and empty text".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![
            vec![null],
            vec![Cell::scalar("")],
            vec![Cell::scalar("NULL")],
        ],
        has_more: false,
    };
    let mut selection = Selection::default();
    let mut output = context.run_ui(input(vec![]), |ui| {
        assert!(
            show(
                ui,
                &data,
                &mut selection,
                Pane::Document,
                Features::default()
            )
            .actions
            .is_empty()
        );
        assert!(selection.copy(&data, ui.ctx()));
    });
    output.textures_delta.clear();
    let nulls: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| {
            let egui::Shape::Text(text) = &shape.shape else {
                return None;
            };
            (text.galley.job.text == "NULL").then_some(text)
        })
        .collect();
    assert_eq!(
        nulls.len(),
        2,
        "empty text does not invent a NULL marker or new row"
    );
    assert!(nulls[0].galley.job.sections[0].format.italics);
    assert_eq!(
        nulls[0].fallback_color,
        crate::theme::palette(&context).muted
    );
    assert!(!nulls[1].galley.job.sections[0].format.italics);
    assert!(
        output
            .platform_output
            .commands
            .contains(&OutputCommand::CopyText(String::new()))
    );
    assert!(data.rows[0][0].text.is_empty());
    assert!(data.rows[1][0].text.is_empty());

    let header = output
        .shapes
        .iter()
        .find_map(|shape| {
            let egui::Shape::Text(text) = &shape.shape else {
                return None;
            };
            (text.galley.job.text == "Value").then_some(text)
        })
        .expect("actual column caption");
    // Center-aligned galleys have a negative local x origin; pos is their anchor.
    let caption_center = header.galley.rect.translate(header.pos.to_vec2()).center();
    let header_rect = output
        .shapes
        .iter()
        .find_map(|shape| {
            let egui::Shape::Rect(rectangle) = &shape.shape else {
                return None;
            };
            (rectangle.fill == crate::theme::palette(&context).grid_header
                && rectangle.rect.contains(caption_center))
            .then_some(rectangle.rect)
        })
        .expect("actual column header background");
    assert!(
        (caption_center.x - header_rect.center().x).abs() <= 1.0,
        "caption={caption_center:?}, header={header_rect:?}"
    );
}

#[test]
fn empty_filtered_body_keeps_headers_anchored_without_inventing_a_record() {
    use egui_kittest::kittest::Queryable;

    let mut header_positions = Vec::new();
    for scene in [
        crate::preview::Scene::Filtered,
        crate::preview::Scene::FilterEmpty,
    ] {
        let mut preview = crate::preview::Preview::new(scene);
        let mut state = std::mem::take(preview.state_mut());
        state.localizer = crate::Localizer::new(crate::Locale::De).unwrap();
        let message = state.localizer.text("datasheet-no-rows");
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 580.0))
            .with_max_steps(32)
            .build_ui_state(
                |ui, state: &mut crate::State| assert!(state.show(ui).is_empty()),
                state,
            );
        crate::configure(&harness.ctx);
        harness.ctx.set_pixels_per_point(1.25);
        harness.run();
        let header = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| {
                let egui::Shape::Text(text) = &shape.shape else {
                    return None;
                };
                (text.galley.job.text == "Customer ID").then_some(text.pos.y)
            })
            .expect("actual first column caption");
        header_positions.push(header);

        if scene == crate::preview::Scene::FilterEmpty {
            let painted = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    let egui::Shape::Text(text) = &shape.shape else {
                        return None;
                    };
                    (text.galley.job.text == message).then_some((shape, text))
                })
                .expect("localized empty-state decoration remains visible");
            let bounds = painted.1.galley.rect.translate(painted.1.pos.to_vec2());
            assert!(bounds.top() > header);
            let insertion_label = harness.state().localizer.text("insertion-title");
            let insertion = harness
                .get_all_by_role_and_label(Role::Button, &insertion_label)
                .into_iter()
                .find(|button| button.rect().top() > header && button.rect().top() < bounds.top())
                .expect("the empty datasheet retains its New row action");
            assert!(bounds.top() >= insertion.rect().bottom());
            assert!(
                painted.0.clip_rect.contains_rect(bounds),
                "empty message={bounds:?}, clip={:?}",
                painted.0.clip_rect,
            );
            assert!(harness.state().data.as_ref().unwrap().rows.is_empty());
            assert!(harness.state().selected_cell_text().is_none());
            assert!(harness.state().current_delete_target().is_none());
        }
    }
    assert_eq!(header_positions[0], header_positions[1]);
}

#[test]
fn final_native_page_add_action_uses_existing_insert_target_without_a_record_identity() {
    use egui_kittest::{Harness, kittest::Queryable};

    for (rows, has_more, source_revision, offered) in [
        (1, false, Some(7), true),
        (0, false, Some(7), true),
        (1, true, Some(7), false),
        (1, false, None, false),
    ] {
        let data = Data {
            query_contents: None,
            row_ordinals: None,
            source_revision,
            table: "Items".into(),
            offset: 0,
            columns: vec!["Id".into()],
            rows: (0..rows).map(|_| vec![Cell::scalar("1")]).collect(),
            has_more,
        };
        let target = InsertTarget {
            document_revision: 7,
            table: "Items".into(),
        };
        let mut selection = Selection::default();
        selection.reset(&data);
        let selected = selection.position();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui_state(
                |ui, state: &mut (Data, Selection, Vec<Action>)| {
                    state.2.extend(
                        show(
                            ui,
                            &state.0,
                            &mut state.1,
                            Pane::Document,
                            Features {
                                insert_target: Some(InsertTarget {
                                    document_revision: 7,
                                    table: "Items".into(),
                                }),
                                ..Features::default()
                            },
                        )
                        .actions,
                    );
                },
                (data, selection, Vec::new()),
            );
        crate::configure(&harness.ctx);
        harness.run();
        if offered {
            harness
                .get_by_role_and_label(Role::Button, "New row")
                .click_accesskit();
            harness.run();
            assert!(matches!(
                harness.state().2.as_slice(),
                [Action::BeginInsert { target: actual }] if actual == &target
            ));
        } else {
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, "New row")
                    .is_none()
            );
            assert!(harness.state().2.is_empty());
        }
        assert_eq!(harness.state().0.rows.len(), rows);
        assert_eq!(harness.state().1.position(), selected);
        assert_eq!(harness.state().0.parent_ordinal(rows), None);
    }
}

#[test]
fn add_column_header_is_disabled_and_never_becomes_a_stored_cell() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };

    let data = Data {
        query_contents: None,
        row_ordinals: None,
        source_revision: Some(7),
        table: "Items".into(),
        offset: 0,
        columns: vec!["Value".into()],
        rows: vec![vec![Cell::scalar("native value")]],
        has_more: false,
    };
    let mut selection = Selection::default();
    selection.reset(&data);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 300.0))
        .build_ui_state(
            |ui, state: &mut (Data, Selection, Vec<Action>)| {
                state.2.extend(
                    show(
                        ui,
                        &state.0,
                        &mut state.1,
                        Pane::Document,
                        Features::default(),
                    )
                    .actions,
                );
            },
            (data, selection, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    let header = harness.get_by_role_and_label(Role::Button, "Click to Add");
    assert!(header.accesskit_node().is_disabled());
    let center = header.rect().center();
    for pressed in [true, false] {
        harness.event(Event::PointerMoved(center));
        harness.event(Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert_eq!(harness.state().1.position(), Some((0, 0)));
    assert_eq!(harness.state().0.columns.len(), 1);
    assert_eq!(harness.state().0.rows[0].len(), 1);
    assert!(harness.state().2.is_empty());

    harness.state_mut().0.source_revision = None;
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Click to Add")
            .is_none(),
        "query results do not advertise native field creation"
    );
}

mod boolean;

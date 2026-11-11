use super::*;
use crate::datasheet::{Selection, show_rows};
use crate::workspace::focus::Pane;
use crate::{Cell, Data};
use egui::{Context, Event, PointerButton, RawInput, Rect};

struct Observation {
    width: f32,
    horizontal: f32,
    first_cell: Option<Rect>,
    header_y: f32,
}

fn frame(
    context: &Context,
    object: u64,
    data: &Data,
    selection: &mut Selection,
    events: Vec<Event>,
) -> Observation {
    let mut observation = None;
    let mut output = context.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 400.0),
            )),
            time: Some(context.cumulative_frame_nr() as f64 * 0.25),
            events,
            ..Default::default()
        },
        |ui| {
            ui.push_id(("object_document", object), |ui| {
                let mut actions = Vec::new();
                let scroll = show_rows(ui, data, |_| {}, &mut actions, selection, Pane::Document);
                assert!(actions.is_empty());
                observation = Some(Observation {
                    width: PageLayout::load(ui, data).width(0).unwrap(),
                    horizontal: scroll.state.offset.x,
                    first_cell: None,
                    header_y: 0.0,
                });
            });
        },
    );
    let mut observed = observation.unwrap();
    let prefix = format!(
        "\u{2068}{}\u{2069}, row \u{2068}{}\u{2069}:",
        data.columns[0].name,
        data.offset + 1
    );
    if let Some(tree) = &output.platform_output.accesskit_update {
        observed.first_cell = tree.nodes.iter().find_map(|(_, node)| {
            if node.value().is_some_and(|value| value.starts_with(&prefix)) {
                node.bounds().map(|bounds| {
                    Rect::from_min_max(
                        egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
                        egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
                    )
                })
            } else {
                None
            }
        });
    }
    observed.header_y = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == data.columns[0].name => {
                Some(text.pos.y + text.galley.size().y * 0.5)
            }
            _ => None,
        })
        .unwrap_or(0.0);
    output.textures_delta.clear();
    observed
}

#[test]
fn pointer_column_resize_and_horizontal_viewport_follow_pages_but_not_other_sources_or_schemas() {
    let context = Context::default();
    context.enable_accesskit();
    crate::configure(&context);
    let mut data = Data {
        query_contents: None,
        source_revision: Some(7),
        row_ordinals: None,
        table: "Items".into(),
        offset: 0,
        columns: (0..10)
            .map(|index| format!("Field {index}").into())
            .collect(),
        rows: vec![
            (0..10)
                .map(|index| Cell::scalar(format!("Literal {index}")))
                .collect(),
        ],
        has_more: true,
    };
    let rows = data.rows.as_ptr();
    let mut selection = Selection::default();
    frame(&context, 1, &data, &mut selection, Vec::new());
    let initial = frame(&context, 1, &data, &mut selection, Vec::new());
    let boundary = egui::pos2(initial.first_cell.unwrap().right(), initial.header_y);
    frame(
        &context,
        1,
        &data,
        &mut selection,
        vec![
            Event::PointerMoved(boundary),
            Event::PointerButton {
                pos: boundary,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    let moved = boundary + egui::vec2(72.0, 0.0);
    frame(
        &context,
        1,
        &data,
        &mut selection,
        vec![Event::PointerMoved(moved)],
    );
    frame(
        &context,
        1,
        &data,
        &mut selection,
        vec![Event::PointerButton {
            pos: moved,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let resized = frame(&context, 1, &data, &mut selection, Vec::new());
    assert!(
        resized.width > initial.width + 60.0,
        "actual pointer drag must resize the column"
    );
    frame(
        &context,
        1,
        &data,
        &mut selection,
        vec![
            Event::PointerMoved(egui::pos2(400.0, 150.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(-160.0, 0.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    // A wheel gesture animates its remaining delta across subsequent frames.
    // Finish that gesture before measuring a separate page transition.
    let mut scrolled = frame(&context, 1, &data, &mut selection, Vec::new());
    for _ in 0..5 {
        scrolled = frame(&context, 1, &data, &mut selection, Vec::new());
    }
    assert!(scrolled.horizontal > 0.0);

    for offset in [100, 0] {
        data.offset = offset;
        selection.reset(&data);
        let page = frame(&context, 1, &data, &mut selection, Vec::new());
        assert!((page.width - resized.width).abs() < 1.0);
        assert!(
            (page.horizontal - scrolled.horizontal).abs() < 1.0,
            "page {offset}: before {}, after {}",
            scrolled.horizontal,
            page.horizontal
        );
        assert_eq!(data.parent_ordinal(0), Some(offset));
    }

    let other_object = frame(&context, 2, &data, &mut selection, Vec::new());
    assert_eq!(other_object.width, initial.width);
    assert_eq!(other_object.horizontal, 0.0);
    for changed in 0..3 {
        match changed {
            0 => data.table = "Other table".into(),
            1 => {
                data.table = "Items".into();
                data.source_revision = Some(8);
            }
            _ => {
                data.source_revision = Some(7);
                data.columns[0].name = "Different field".into();
            }
        }
        let separate = frame(&context, 1, &data, &mut selection, Vec::new());
        assert_eq!(separate.width, initial.width);
        assert_eq!(separate.horizontal, 0.0);
    }
    data.columns[0].name = "Field 0".into();
    let original = frame(&context, 1, &data, &mut selection, Vec::new());
    assert!((original.width - resized.width).abs() < 1.0);
    assert!((original.horizontal - scrolled.horizontal).abs() < 1.0);
    assert_eq!(data.rows.as_ptr(), rows);
    assert_eq!(data.rows[0][0].text, "Literal 0");
}

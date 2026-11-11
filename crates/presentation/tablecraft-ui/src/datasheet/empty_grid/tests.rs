use super::*;
use crate::datasheet::{Selection, show};
use crate::workspace::focus::Pane;
use crate::workspace::totals::Row;
use crate::{Cell, Data, InsertTarget, TotalsTarget};
use egui::{Event, PointerButton, Pos2, RawInput, Vec2, accesskit::Role};

#[test]
fn filler_rules_follow_scaled_rows_without_creating_selectable_records() {
    for (rows, offer_insert, totals) in [(0, false, false), (2, true, false), (2, true, true)] {
        let context = egui::Context::default();
        context.enable_accesskit();
        crate::configure(&context);
        crate::theme::set_datasheet_zoom(&context, 150);
        let data = Data {
            query_contents: None,
            row_ordinals: None,
            source_revision: Some(7),
            table: "Items".into(),
            offset: 100,
            columns: vec!["Value".into(), "Other".into()],
            rows: (0..rows)
                .map(|row| vec![Cell::scalar(row.to_string()), Cell::scalar("literal")])
                .collect(),
            has_more: false,
        };
        let target = TotalsTarget {
            document_revision: 7,
            table: "Items".into(),
            filter_id: None,
            enabled: false,
        };
        let mut selection = Selection::default();
        selection.reset(&data);
        let before = selection.position();
        let mut events = Vec::new();
        let mut grid_bounds = None;
        for frame in 0..4 {
            let mut output = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 360.0))),
                    events: std::mem::take(&mut events),
                    ..Default::default()
                },
                |ui| {
                    let output = show(
                        ui,
                        &data,
                        &mut selection,
                        Pane::Document,
                        crate::datasheet::Features {
                            insert_target: offer_insert.then_some(InsertTarget {
                                document_revision: 7,
                                table: "Items".into(),
                            }),
                            totals: totals.then_some(Row {
                                target: &target,
                                fields: None,
                                specs: &[],
                                result: None,
                                enabled: false,
                                pending: false,
                                error: false,
                                selected_column: None,
                            }),
                            ..crate::datasheet::Features::default()
                        },
                    );
                    assert!(output.actions.is_empty());
                    assert!(output.totals_choices.is_empty());
                },
            );
            output.textures_delta.clear();
            let color = crate::theme::palette(&context)
                .grid_line
                .gamma_multiply(0.4);
            let lines = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    Shape::Vec(lines)
                        if lines.iter().any(|line| {
                            matches!(line, Shape::LineSegment { stroke, .. } if stroke.color == color)
                        }) => Some(lines),
                    _ => None,
                })
                .expect("the unoccupied viewport has a background grid");
            let horizontal: Vec<_> = lines
                .iter()
                .filter_map(|line| match line {
                    Shape::LineSegment { points, .. } if points[0].y == points[1].y => {
                        Some(*points)
                    }
                    _ => None,
                })
                .collect();
            assert!(horizontal.len() > 3);
            assert!(
                horizontal
                    .windows(2)
                    .all(|pair| { (pair[1][0].y - pair[0][0].y - 30.0).abs() < 0.01 })
            );
            let first = horizontal.first().unwrap();
            let last = horizontal.last().unwrap();
            assert!(first[0].y >= 33.0 + (rows + usize::from(offer_insert)) as f32 * 30.0);
            if frame == 0 {
                let tree = output.platform_output.accesskit_update.as_ref().unwrap();
                assert_eq!(
                    tree.nodes
                        .iter()
                        .filter(|(_, node)| node.role() == Role::RowHeader)
                        .count(),
                    rows,
                    "filler rules must not expose extra records"
                );
            }
            let bounds = Rect::from_min_max(first[0], last[1]);
            grid_bounds = Some(bounds);
            let pointer = pos2(bounds.left() + 50.0, bounds.bottom() - 5.0);
            if frame == 1 || frame == 2 {
                events = vec![
                    Event::PointerMoved(pointer),
                    Event::PointerButton {
                        pos: pointer,
                        button: PointerButton::Primary,
                        pressed: frame == 1,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
            }
        }
        assert_eq!(selection.position(), before);
        assert!(grid_bounds.unwrap().height() > 100.0);
        assert_eq!(data.rows.len(), rows);
    }
}

#[test]
fn scrolled_geometry_keeps_row_phase_and_clips_to_the_visible_columns() {
    let viewport = Rect::from_min_max(pos2(25.0, 100.0), pos2(200.0, 230.0));
    let columns = [
        Rect::from_min_max(pos2(-40.0, 0.0), pos2(-10.0, 20.0)),
        Rect::from_min_max(pos2(-10.0, 0.0), pos2(140.0, 20.0)),
        Rect::from_min_max(pos2(140.0, 0.0), pos2(290.0, 20.0)),
    ];
    let Shape::Vec(lines) = shape(&columns, viewport, 83.0, 30.0, Color32::GRAY) else {
        panic!("visible unoccupied space must have rules");
    };
    for line in &lines {
        let Shape::LineSegment { points, .. } = line else {
            panic!("decoration contains line segments only");
        };
        assert!(points.iter().all(|point| viewport.contains(*point)));
    }
    let horizontal: Vec<_> = lines
        .iter()
        .filter_map(|line| match line {
            Shape::LineSegment { points, .. } if points[0].y == points[1].y => Some(points[0].y),
            _ => None,
        })
        .collect();
    assert_eq!(horizontal, [113.0, 143.0, 173.0, 203.0]);
    assert!(matches!(
        shape(&columns, viewport, 250.0, 30.0, Color32::GRAY),
        Shape::Noop
    ));
}

use crate::datasheet::{Selection, show_rows};
use crate::workspace::focus::Pane;
use crate::{Cell, Data, RowOrdinals};
use egui::{Context, RawInput, Rect, Vec2, accesskit::Role};

#[test]
fn virtualized_grid_exposes_real_rows_fields_and_existing_value_widgets() {
    for has_more in [true, false] {
        let context = Context::default();
        context.enable_accesskit();
        let data = Data {
            query_contents: None,
            source_revision: Some(1),
            row_ordinals: Some(
                RowOrdinals::from_display_order((0..10_000).rev().map(|index| index * 2).collect())
                    .expect("independent sparse sorted membership"),
            ),
            table: "Records".into(),
            offset: 100,
            columns: vec!["Name".into()],
            rows: (0..10_000)
                .map(|index| vec![Cell::scalar(format!("Value {index}"))])
                .collect(),
            has_more,
        };
        let mut drawn = Vec::new();
        let mut output = context.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(800.0, 400.0),
                )),
                ..Default::default()
            },
            |ui| {
                show_rows(
                    ui,
                    &data,
                    |index| drawn.push(index),
                    &mut Vec::new(),
                    &mut Selection::default(),
                    Pane::Document,
                );
            },
        );
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.expect("grid tree");
        let nodes = &tree.nodes;
        let (grid_id, grid) = nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Grid)
            .expect("grid");
        let grid_bounds = grid.bounds().expect("grid bounds");
        assert!(grid_bounds.width() > 0.0 && grid_bounds.height() > 0.0);
        assert_eq!(grid.label(), Some("Records"));
        assert_eq!(grid.column_count(), Some(2));
        assert_eq!(grid.row_count(), (!has_more).then_some(10_101));
        assert!(!drawn.is_empty() && drawn.len() < 30);

        let rows: Vec<_> = nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Row)
            .collect();
        assert_eq!(
            rows.len(),
            drawn.len() + 1,
            "header and rendered records only"
        );
        for (id, row) in rows {
            assert!(
                grid.children().contains(id),
                "row must be a grid child: {grid_id:?}"
            );
            let row_bounds = row.bounds().expect("actual row bounds");
            assert!(row_bounds.width() > 0.0 && row_bounds.height() > 0.0);
            assert!(
                row_bounds.height() <= 22.0,
                "row must not cover the viewport"
            );
            let index = row.row_index().expect("zero-based row index");
            if index == 0 {
                assert!(nodes.iter().any(|(id, node)| row.children().contains(id)
                    && node.role() == Role::ColumnHeader
                    && node.column_index() == Some(1)));
                continue;
            }
            assert!(drawn.contains(&(index - 101)));
            let (cell_id, cell) = nodes
                .iter()
                .find(|(id, node)| row.children().contains(id) && node.role() == Role::GridCell)
                .expect("field container");
            assert_eq!(cell.column_index(), Some(1));
            let selector_container = nodes
                .iter()
                .find(|(id, node)| row.children().contains(id) && node.column_index() == Some(0))
                .expect("selector container");
            assert!(
                nodes
                    .iter()
                    .any(|(id, node)| selector_container.1.children().contains(id)
                        && node.role() == Role::RowHeader
                        && node.column_index() == Some(0))
            );

            assert!(
                nodes.iter().any(|(id, node)| cell.children().contains(id)
                    && node.role() == Role::Label
                    && (node.value().is_some() || node.label().is_some())),
                "original value widget under {cell_id:?}"
            );
        }
    }
}

#[test]
fn empty_nonzero_page_does_not_invent_a_relation_length() {
    for (offset, expected) in [(0, Some(1)), (100, None)] {
        let context = Context::default();
        context.enable_accesskit();
        let data = Data {
            query_contents: None,
            source_revision: None,
            row_ordinals: None,
            table: "Empty page".into(),
            offset,
            columns: vec!["Name".into()],
            rows: Vec::new(),
            has_more: false,
        };
        let mut output = context.run_ui(RawInput::default(), |ui| {
            super::grid(ui, &data);
        });
        output.textures_delta.clear();
        let tree = output.platform_output.accesskit_update.expect("grid tree");
        let (_, grid) = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Grid)
            .expect("grid");
        assert_eq!(grid.row_count(), expected);
    }
}

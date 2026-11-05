use super::*;
use crate::Theme;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::query::{DesignFieldMetadata, DesignTableMetadata};

fn session() -> Session {
    let mut session = Session::default();
    for (id, table, field) in [
        (1, "Customers", "CustomerID"),
        (2, "Orders", "OrderCustomer"),
    ] {
        session.draft.sources.push(DesignSource {
            id,
            table: table.into(),
            alias: None,
        });
        session.metadata.push(DesignTableMetadata {
            table: table.into(),
            fields: vec![DesignFieldMetadata {
                name: field.into(),
                scalar_type: None,
            }],
        });
    }
    session.draft.joins.push(DesignJoin {
        left: DesignField {
            source_id: 1,
            column: "CustomerID".into(),
        },
        right: DesignField {
            source_id: 2,
            column: "OrderCustomer".into(),
        },
        kind: DesignJoinKind::Inner,
    });
    session
}

#[test]
fn source_cards_keep_compact_vertical_fields_and_join_edges_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        let mut harness = Harness::builder()
            .with_step_dt(1.0 / 60.0)
            .with_size(egui::vec2(600.0, 220.0))
            .build_ui_state(
                |ui, session: &mut Session| {
                    source_area(ui, session, 140.0);
                },
                session(),
            );
        crate::configure(&harness.ctx);
        crate::theme::set_theme(&harness.ctx, theme);
        harness.run();
        let title = harness.get_by_label("Customers").rect();
        let left = harness.get_by_label("CustomerID").rect();
        let right = harness.get_by_label("OrderCustomer").rect();
        assert!(left.top() > title.bottom());
        assert!((left.width() - SOURCE_WIDTH).abs() < 1.0);
        assert!((left.height() - FIELD_ROW_HEIGHT).abs() < 1.0);
        assert!(right.left() - left.right() >= CARD_GAP);
        let connector = harness.output().shapes.iter().find_map(|shape| {
            let egui::Shape::Path(path) = &shape.shape else {
                return None;
            };
            (path.points.len() == 4
                && path.points[0] == left.right_center()
                && path.points[3] == right.left_center())
            .then_some((shape.clip_rect, &path.points))
        });
        let (clip, points) = connector.expect("only the existing declared join is drawn");
        assert!(points.iter().all(|point| clip.contains(*point)));
        assert!(harness.state().draft.columns.is_empty());
        assert_eq!(harness.state().draft.joins.len(), 1);

        harness.get_by_label("CustomerID").click();
        harness.run_steps(1);
        assert!(harness.state().draft.columns.is_empty());
        harness.get_by_label("CustomerID").click();
        harness.run_steps(1);
        assert_eq!(harness.state().draft.columns.len(), 1);
        harness.get_by_label("CustomerID").click();
        harness.run_steps(1);
        assert_eq!(harness.state().draft.columns.len(), 1);
        assert_eq!(
            harness.state().draft.columns[0].expression,
            DesignExpression::Field(DesignField {
                source_id: 1,
                column: "CustomerID".into(),
            })
        );
    }
}

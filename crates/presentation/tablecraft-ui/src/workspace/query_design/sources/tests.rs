use super::*;
use crate::Theme;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::query::{DesignFieldMetadata, DesignTableMetadata};

pub(super) fn session() -> Session {
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

#[test]
fn bounded_source_card_scroll_reaches_all_fields_without_moving_the_canvas_or_draft() {
    let mut source = session();
    source.metadata[0]
        .fields
        .extend((0..30).map(|index| DesignFieldMetadata {
            name: format!("Synthetic field {index:02}"),
            scalar_type: None,
        }));
    let original = source.draft.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(600.0, 400.0))
        .build_ui_state(
            |ui, session: &mut Session| {
                source_area(ui, session, 300.0);
            },
            source,
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    let title = harness.get_by_label("Customers").rect();
    let field = harness.get_by_label("CustomerID").rect();
    let canvas = harness
        .output()
        .shapes
        .iter()
        .find_map(|shape| {
            let egui::Shape::Rect(rectangle) = &shape.shape else {
                return None;
            };
            (rectangle.fill == crate::theme::palette(&harness.ctx).title_fill
                && rectangle.rect.width() > 500.0
                && rectangle.rect.height() >= 290.0)
                .then_some(rectangle.rect)
        })
        .expect("the complete upper source canvas has a theme-aware gray fill");
    harness.event(egui::Event::PointerMoved(field.center()));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -10000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    harness.run_steps(4);
    let last = harness.get_by_label("Synthetic field 29").rect();
    assert_eq!(harness.get_by_label("Customers").rect(), title);
    assert!(last.top() >= title.bottom());
    assert!(last.bottom() < title.top() + 165.0);
    assert!(canvas.contains_rect(last));
    assert_eq!(harness.state().draft, original);
}

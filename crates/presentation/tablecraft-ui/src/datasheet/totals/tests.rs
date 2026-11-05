use crate::datasheet::{Selection, show};
use crate::workspace::focus::Pane;
use crate::workspace::totals::{Choice, Row};
use crate::{
    Cell, Data, InsertTarget, TotalAggregate, TotalSpec, TotalsData, TotalsField, TotalsTarget,
};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn derived_row_is_reachable_and_column_aligned_without_becoming_a_record() {
    for (rows, offer_insert, eligible) in [
        (0, true, true),
        (0, false, true),
        (2, true, true),
        (100, true, true),
        (2, true, false),
    ] {
        let data = Data {
            row_ordinals: None,
            source_revision: Some(7),
            table: "Items".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: (0..rows)
                .map(|row| vec![Cell::scalar(row.to_string())])
                .collect(),
            has_more: false,
        };
        let target = TotalsTarget {
            document_revision: 7,
            table: "Items".into(),
            filter_id: None,
            enabled: false,
        };
        let specs = vec![TotalSpec {
            column: "Value".into(),
            aggregate: TotalAggregate::Sum,
        }];
        let fields = vec![TotalsField {
            column: "Value".into(),
            aggregates: if eligible {
                vec![TotalAggregate::Count, TotalAggregate::Sum]
            } else {
                Vec::new()
            },
        }];
        let result = TotalsData {
            target: target.clone(),
            specs: specs.clone(),
            cells: vec![Cell::scalar("123")],
            source_rows: rows as u32,
            view_rows: rows as u32,
        };
        let mut selection = Selection::default();
        selection.reset(&data);
        let selected = selection.position();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui_state(
                move |ui,
                      state: &mut (
                    Data,
                    Selection,
                    Vec<Choice>,
                    Option<crate::datasheet::SelectionOwner>,
                )| {
                    let output = show(
                        ui,
                        &state.0,
                        &mut state.1,
                        Pane::Document,
                        None,
                        offer_insert.then_some(InsertTarget {
                            document_revision: 7,
                            table: "Items".into(),
                        }),
                        Some(Row {
                            target: &target,
                            fields: Some(&fields),
                            specs: &specs,
                            result: eligible.then_some(&result),
                            enabled: true,
                            pending: false,
                            error: !eligible,
                            selected_column: None,
                        }),
                    );
                    assert!(output.actions.is_empty());
                    state.2.extend(output.totals_choices);
                    state.3 = output.selection;
                },
                (data, selection, Vec::new(), None),
            );
        crate::configure(&harness.ctx);
        harness.run();
        let mut arguments = crate::FluentArgs::new();
        arguments.set("column", "Value");
        let selector_label =
            crate::localization::for_context(&harness.ctx).format("totals-column", &arguments);
        let selector = harness.get_by_role_and_label(Role::ComboBox, &selector_label);
        let selector_rect = selector.rect();
        assert!(selector_rect.bottom() <= 300.0);
        if rows == 0 {
            let message = crate::localization::for_context(&harness.ctx).text("datasheet-no-rows");
            let (shape, text) = harness
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    let egui::Shape::Text(text) = &shape.shape else {
                        return None;
                    };
                    (text.galley.job.text == message).then_some((shape, text))
                })
                .expect("the empty-state decoration remains visible alongside Totals");
            let bounds = text.galley.rect.translate(text.pos.to_vec2());
            assert!(bounds.top() >= selector_rect.bottom());
            assert!(shape.clip_rect.contains_rect(bounds));
        }
        let caption = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| {
                let egui::Shape::Text(text) = &shape.shape else {
                    return None;
                };
                (text.galley.job.text == "Value").then_some(text)
            })
            .unwrap();
        let caption_center = caption
            .galley
            .rect
            .translate(caption.pos.to_vec2())
            .center();
        let header = harness
            .output()
            .shapes
            .iter()
            .find_map(|shape| {
                let egui::Shape::Rect(rectangle) = &shape.shape else {
                    return None;
                };
                (rectangle.fill == crate::theme::palette(&harness.ctx).grid_header
                    && rectangle.rect.contains(caption_center))
                .then_some(rectangle.rect)
            })
            .expect("actual native column header bounds");
        assert!(selector_rect.top() >= header.bottom());
        if rows < 100 {
            let content_bottom = header.bottom() + (rows + usize::from(offer_insert)) as f32 * 20.0;
            assert!((selector_rect.top() - content_bottom).abs() <= 1.0);
        }
        assert!((selector_rect.center().x - caption_center.x).abs() <= 2.0);
        selector.focus();
        harness.run();
        assert!(matches!(
            harness.state().3.as_ref(),
            Some(crate::datasheet::SelectionOwner::Total { target, column })
                if target.document_revision == 7 && column == "Value"
        ));
        if eligible {
            harness.event(egui::Event::Copy);
            harness.run_steps(1);
            assert!(
                harness
                    .output()
                    .platform_output
                    .commands
                    .iter()
                    .any(|command| {
                        matches!(command, egui::OutputCommand::CopyText(value) if value == "123")
                    })
            );
        }
        harness
            .get_by_role_and_label(Role::ComboBox, &selector_label)
            .click_accesskit();
        harness.run();
        let expected = if eligible {
            harness.get_by_label("Count").click_accesskit();
            Some(TotalAggregate::Count)
        } else {
            assert!(harness.query_by_label("Count").is_none());
            assert!(harness.query_by_label("Sum").is_none());
            harness.get_by_label("None").click_accesskit();
            None
        };
        harness.run();
        assert!(matches!(
            harness.state().2.as_slice(),
            [Choice { column, aggregate, .. }] if column == "Value" && *aggregate == expected
        ));
        assert_eq!(harness.state().0.rows.len(), rows);
        assert_eq!(harness.state().1.position(), selected);
        assert_eq!(harness.state().0.parent_ordinal(rows), None);
    }
}

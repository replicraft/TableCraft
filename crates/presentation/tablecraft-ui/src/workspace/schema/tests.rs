use super::*;
use crate::Data;
use crate::workspace::DocumentView;
use egui::{Event, FullOutput, PointerButton};

fn field(name: &str, number: usize, scalar_type: Option<&str>) -> Field {
    Field {
        name: name.into(),
        number,
        native_type: 0x99,
        storage_bytes: 16,
        fixed: true,
        flags: 3,
        extended_flags: 0,
        scalar_type: scalar_type.map(str::to_owned),
        reader_nullable: None,
        decimal_parameters: None,
        complex_type_id: None,
        properties: Vec::new(),
    }
}

fn schema() -> Schema {
    Schema {
        table: "Mixed".into(),
        declared_rows: 42,
        fields: vec![
            field("Original unsupported name", 7, None),
            field("Amount", 2, Some("Int32")),
        ],
    }
}

fn render(state: &mut State, context: &egui::Context, events: Vec<Event>) -> FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 1024.0),
            )),
            time: Some(context.cumulative_frame_nr() as f64 * 0.25),
            events,
            ..Default::default()
        },
        |ui| {
            assert!(state.show(ui).is_empty());
        },
    );
    output.textures_delta.clear();
    output
}

fn painted(output: &FullOutput, expected: &str) -> bool {
    output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == expected)
    })
}

#[test]
fn observed_properties_are_read_only_and_follow_actual_field_selection() {
    const DEFAULT_LITERAL: &str = "Eval(\"DELETE FROM Items\")";

    let context = egui::Context::default();
    context.enable_accesskit();
    let mut declaration = schema();
    declaration.fields[0].properties = vec![
        FieldProperty {
            name: "Required",
            value: FieldPropertyValue::Boolean(true),
        },
        FieldProperty {
            name: "AllowZeroLength",
            value: FieldPropertyValue::Boolean(false),
        },
        FieldProperty {
            name: "DefaultValue",
            value: FieldPropertyValue::Text(DEFAULT_LITERAL.into()),
        },
        FieldProperty {
            name: "ValidationRule",
            value: FieldPropertyValue::Absent,
        },
        FieldProperty {
            name: "ValidationText",
            value: FieldPropertyValue::Uninspected,
        },
        FieldProperty {
            name: "Description",
            value: FieldPropertyValue::Unsupported,
        },
    ];
    declaration.fields[1].properties = vec![FieldProperty {
        name: "DefaultValue",
        value: FieldPropertyValue::Text("Other stored default".into()),
    }];
    let mut state = State::default();
    state.prepare_schema_view();
    state.set_schema(declaration);
    state.dismiss_backstage();

    let output = render(&mut state, &context, vec![]);
    for value in [
        "General",
        "Yes",
        "No",
        DEFAULT_LITERAL,
        "Not stored",
        "Not inspected",
        "Unsupported value",
        "Native details",
        "Allow Zero Length",
        "Default Value",
        "Validation Rule",
        "Validation Text",
    ] {
        assert!(painted(&output, value), "missing observation: {value}");
    }
    assert!(!painted(&output, "Native flags"));
    assert!(!painted(&output, "Lookup"));
    for key in [
        "AllowZeroLength",
        "DefaultValue",
        "ValidationRule",
        "ValidationText",
    ] {
        assert!(!painted(&output, key), "painted canonical key: {key}");
    }
    assert_eq!(
        state.schema.as_ref().unwrap().fields[0].properties[2].name,
        "DefaultValue"
    );
    let tree = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual property accessibility tree");
    let property_name = tree
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.role() == egui::accesskit::Role::Label && node.value() == Some("Default Value"))
                .then_some(*id)
        })
        .expect("actual property label");
    assert!(tree.nodes.iter().any(|(_, node)| {
        node.role() == egui::accesskit::Role::Label
            && node.value() == Some(DEFAULT_LITERAL)
            && node.labelled_by().contains(&property_name)
    }));

    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "Amount" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("actual field-selection label");
    render(
        &mut state,
        &context,
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
        &mut state,
        &context,
        vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    let output = render(&mut state, &context, vec![]);
    assert_eq!(state.selected_field, 1);
    assert!(painted(&output, "Other stored default"));
    assert!(!painted(&output, DEFAULT_LITERAL));
}

#[test]
fn design_publication_and_source_reset_preserve_independent_result_slots() {
    let mut state = State::default();
    state.set_table_data(Data {
        source_revision: None,
        table: "Records".into(),
        offset: 100,
        columns: vec![],
        rows: vec![],
        has_more: false,
    });
    state.set_query_data(Data {
        source_revision: None,
        table: "Query result".into(),
        offset: 0,
        columns: vec![],
        rows: vec![],
        has_more: false,
    });
    state.prepare_schema_view();
    state.set_schema(schema());
    state.dismiss_backstage();
    assert!(state.view == DocumentView::Design);
    assert!(!state.query_ready());
    assert!(state.visible_data().is_none());
    state.selected_field = 1;
    assert_eq!(
        state.schema.as_ref().unwrap().fields[state.selected_field].number,
        2
    );
    state.view = DocumentView::Table;
    assert_eq!(state.visible_data().unwrap().offset, 100);
    state.view = DocumentView::Query;
    assert_eq!(state.visible_data().unwrap().table, "Query result");
    state.prepare_schema_view();
    state.set_schema(schema());
    state.dismiss_backstage();
    assert_eq!(state.selected_field, 0);
    state.clear_schema();
    assert!(state.schema.is_none());
    assert!(state.view == DocumentView::Table);
    assert_eq!(state.data.as_ref().unwrap().offset, 100);
}

#[test]
fn late_schema_reply_preserves_the_more_recent_document_choice() {
    let mut state = State::default();
    state.prepare_schema_view();
    state.select_view(DocumentView::Query);
    state.set_schema(schema());
    state.dismiss_backstage();
    assert!(state.view == DocumentView::Query);
    assert_eq!(state.schema.as_ref().unwrap().table, "Mixed");

    state.prepare_schema_view();
    state.set_schema(schema());
    state.dismiss_backstage();
    assert!(state.view == DocumentView::Design);
    state.prepare_schema_view();
    state.select_view(DocumentView::Table);
    state.set_schema(schema());
    state.dismiss_backstage();
    assert!(state.view == DocumentView::Table);

    state.prepare_schema_view();
    state.clear_schema();
    state.set_schema(schema());
    state.dismiss_backstage();
    assert!(state.view == DocumentView::Table);
}

#[test]
fn design_render_is_inert_for_unsupported_and_empty_field_declarations() {
    let ctx = egui::Context::default();
    let mut state = State::default();
    state.prepare_schema_view();
    state.set_schema(schema());
    state.dismiss_backstage();
    for empty in [false, true] {
        if empty {
            state.schema.as_mut().unwrap().fields.clear();
        }
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 820.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(state.show(ui).is_empty());
            },
        );
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
    assert!(state.schema.as_ref().unwrap().fields.is_empty());
}

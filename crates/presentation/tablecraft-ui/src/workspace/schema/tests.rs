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
        display_type: None,
        field_size: None,
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
    render_at_size(state, context, events, egui::vec2(1280.0, 1024.0))
}

fn render_at_size(
    state: &mut State,
    context: &egui::Context,
    events: Vec<Event>,
    size: egui::Vec2,
) -> FullOutput {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
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

fn visible_text_rect(output: &FullOutput, expected: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::epaint::Shape::Text(text) if text.galley.job.text == expected => {
            let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
            shape.clip_rect.contains_rect(rect).then_some(rect)
        }
        _ => None,
    })
}

fn click_label(state: &mut State, context: &egui::Context, label: &str) {
    let output = render(state, context, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("actual declaration or property label");
    for pressed in [true, false] {
        render(
            state,
            context,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn field_key(
    state: &mut State,
    context: &egui::Context,
    key: egui::Key,
    pressed: bool,
    modifiers: egui::Modifiers,
) -> FullOutput {
    render(
        state,
        context,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }],
    )
}

fn assert_field_node(
    output: &FullOutput,
    name: &str,
    selected: bool,
    disabled: bool,
) -> egui::accesskit::NodeId {
    let (id, node) = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("actual declaration accessibility tree")
        .nodes
        .iter()
        .find_map(|(id, node)| (node.label() == Some(name)).then_some((id, node)))
        .expect("actual field-name selection node");
    assert_eq!(node.is_selected(), Some(selected));
    assert_eq!(node.toggled(), None);
    assert_eq!(node.is_disabled(), disabled);
    for action in [
        egui::accesskit::Action::Click,
        egui::accesskit::Action::Focus,
    ] {
        assert_eq!(node.supports_action(action), !disabled);
    }
    *id
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
        FieldProperty {
            name: "Caption",
            value: FieldPropertyValue::Text("Original field display name".into()),
        },
    ];
    declaration.fields[0].field_size = Some("Long Integer".into());
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
    assert_field_node(&output, "Original unsupported name", true, false);
    assert_field_node(&output, "Amount", false, false);
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
    let property_value = tree
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.role() == egui::accesskit::Role::Label
                && node.value() == Some(DEFAULT_LITERAL)
                && node.labelled_by().contains(&property_name))
            .then_some(node)
        })
        .expect("stored expression retains its actual property label");
    let value_bounds = property_value.bounds().expect("property value bounds");
    assert!(
        value_bounds.width() > 600.0,
        "property values use the available pane width"
    );
    assert!(
        value_bounds.height() <= 20.0,
        "property rows retain compact one-line height"
    );
    let value_text_x = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == DEFAULT_LITERAL => {
                Some(text.pos.x)
            }
            _ => None,
        })
        .expect("actual stored-expression paint position");
    // The accessible value region spans the padded remainder column. Its text
    // must begin at that left edge rather than at the center of the wide cell.
    assert!(
        (f64::from(value_text_x) - value_bounds.x0).abs() < 1.0,
        "property text begins at the value-column padding"
    );

    let compact_size = egui::vec2(1280.0, 820.0);
    render_at_size(&mut state, &context, vec![], compact_size);
    let compact = render_at_size(&mut state, &context, vec![], compact_size);
    let required = visible_text_rect(&compact, "Required").expect("visible property-pane row");
    assert!(visible_text_rect(&compact, "Native details").is_none());
    let caption = visible_text_rect(&compact, "Field Properties").expect("property-pane caption");
    assert!(
        compact
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| {
                node.role() == egui::accesskit::Role::ScrollBar
                    && node
                        .bounds()
                        .is_some_and(|bounds| bounds.y0 >= f64::from(caption.bottom()))
            }),
        "overflow has an actual discoverable property-pane scrollbar"
    );

    render_at_size(
        &mut state,
        &context,
        vec![
            Event::PointerMoved(required.center()),
            Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -500.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        compact_size,
    );
    render_at_size(&mut state, &context, vec![], compact_size);
    let scrolled = render_at_size(&mut state, &context, vec![], compact_size);
    assert!(visible_text_rect(&scrolled, "Validation Text").is_some());
    assert!(visible_text_rect(&scrolled, "Native details").is_some());
    assert!(
        !painted(&scrolled, "Native flags"),
        "native details remain collapsed"
    );
    assert_eq!(state.selected_field, 0);

    click_label(&mut state, &context, "Amount");
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

#[test]
fn field_arrows_keep_focus_and_reveal_current_declarations_without_host_actions() {
    let context = egui::Context::default();
    let mut declaration = schema();
    declaration.fields.extend(
        (2..40).map(|position| field(&format!("Field {position}"), position, Some("Int32"))),
    );
    // Public presentation metadata need not have unique native numbers or names.
    // These two rows must remain separately focusable in the current declaration.
    for position in [38, 39] {
        declaration.fields[position].name = "Repeated field".into();
        declaration.fields[position].number = 7;
    }
    for (position, field) in declaration.fields.iter_mut().enumerate() {
        field.properties.push(FieldProperty {
            name: "DefaultValue",
            value: FieldPropertyValue::Text(format!("literal for field {position}")),
        });
    }
    let mut state = State::default();
    state.prepare_schema_view();
    state.set_schema(declaration);
    state.dismiss_backstage();
    click_label(&mut state, &context, "Original unsupported name");

    for pressed in [true, false] {
        field_key(
            &mut state,
            &context,
            egui::Key::ArrowUp,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 0);
    }
    for expected in 1..40 {
        // Keydown and keyup are consecutive native-like frames; no idle frame
        // hides focus changes or scrolling between consecutive keys.
        for pressed in [true, false] {
            field_key(
                &mut state,
                &context,
                egui::Key::ArrowDown,
                pressed,
                egui::Modifiers::NONE,
            );
            assert_eq!(state.selected_field, expected);
        }
    }
    let last_focus = context
        .memory(|memory| memory.focused())
        .expect("last field has focus");
    for pressed in [true, false] {
        let output = field_key(
            &mut state,
            &context,
            egui::Key::ArrowDown,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 39);
        assert!(painted(&output, "literal for field 39"));
    }
    for pressed in [true, false] {
        field_key(
            &mut state,
            &context,
            egui::Key::ArrowUp,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 38);
    }
    assert_ne!(context.memory(|memory| memory.focused()), Some(last_focus));
    for pressed in [true, false] {
        field_key(
            &mut state,
            &context,
            egui::Key::ArrowUp,
            pressed,
            egui::Modifiers::CTRL,
        );
        assert_eq!(state.selected_field, 38);
    }
}

#[test]
fn replaced_schema_and_property_text_do_not_retarget_old_field_focus() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut declaration = schema();
    declaration.fields[0].properties.push(FieldProperty {
        name: "DefaultValue",
        value: FieldPropertyValue::Text("first stored literal".into()),
    });
    let mut state = State::default();
    state.prepare_schema_view();
    state.set_schema(declaration);
    state.dismiss_backstage();
    click_label(&mut state, &context, "Amount");
    assert_eq!(state.selected_field, 1);

    let mut replacement = schema();
    replacement.fields[0].properties.push(FieldProperty {
        name: "DefaultValue",
        value: FieldPropertyValue::Text("replacement stored literal".into()),
    });
    state.set_schema(replacement);
    for pressed in [true, false] {
        field_key(
            &mut state,
            &context,
            egui::Key::ArrowDown,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 0);
    }

    click_label(&mut state, &context, "Original unsupported name");
    click_label(&mut state, &context, "replacement stored literal");
    for pressed in [true, false] {
        let output = field_key(
            &mut state,
            &context,
            egui::Key::ArrowDown,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 0);
        assert!(painted(&output, "replacement stored literal"));
    }

    click_label(&mut state, &context, "Original unsupported name");
    state.schema.as_mut().unwrap().fields[0].name = "A replacement field name".into();
    for pressed in [true, false] {
        field_key(
            &mut state,
            &context,
            egui::Key::ArrowDown,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 0);
    }

    click_label(&mut state, &context, "A replacement field name");
    // A foreground presentation modal disables the Design surface regardless of
    // whether the host has a writable table. No runtime capability is created.
    state.edit = Some(crate::workspace::editing::EditDialog::Checking {
        target: crate::EditTarget {
            document_revision: 1,
            table: "Mixed".into(),
            column: "A replacement field name".into(),
            parent_ordinal: 0,
        },
        request_id: 17,
        error: None,
    });
    let mut disabled_target = None;
    for pressed in [true, false] {
        let output = field_key(
            &mut state,
            &context,
            egui::Key::ArrowDown,
            pressed,
            egui::Modifiers::NONE,
        );
        assert_eq!(state.selected_field, 0);
        if pressed {
            assert_field_node(&output, "A replacement field name", true, true);
            disabled_target = Some(assert_field_node(&output, "Amount", false, true));
        }
    }
    let disabled_target = disabled_target.expect("actual disabled field target");
    for action in [
        egui::accesskit::Action::Click,
        egui::accesskit::Action::Focus,
    ] {
        let output = render(
            &mut state,
            &context,
            vec![Event::AccessKitActionRequest(
                egui::accesskit::ActionRequest {
                    action,
                    target_node: disabled_target,
                    target_tree: egui::accesskit::TreeId::ROOT,
                    data: None,
                },
            )],
        );
        assert_eq!(state.selected_field, 0);
        assert_field_node(&output, "Amount", false, true);
    }
}

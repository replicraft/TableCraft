use super::*;

#[test]
fn property_help_preserves_uninspected_lookup_and_distinct_native_required_semantics() {
    let mut selection = PropertySelection::default();
    assert_eq!(help_key(&selection), "design-help-selection");

    selection.selected = Some("Required".into());
    assert_eq!(help_key(&selection), "design-help-required");
    selection.selected = Some("Reader NULL support".into());
    assert_eq!(help_key(&selection), "design-help-selection");

    selection.lookup = true;
    selection.selected = Some("Required".into());
    assert_eq!(help_key(&selection), "design-lookup-uninspected");
}

#[test]
fn property_sheet_selection_only_changes_help_and_lookup_never_invents_settings() {
    let field = Field {
        name: "Literal field".into(),
        number: 4,
        native_type: 10,
        storage_bytes: 100,
        fixed: false,
        flags: 0,
        extended_flags: 0,
        scalar_type: Some("Text".into()),
        display_type: None,
        field_size: Some("50".into()),
        reader_nullable: Some(true),
        decimal_parameters: None,
        complex_type_id: None,
        properties: vec![
            FieldProperty {
                name: "Required",
                value: FieldPropertyValue::Boolean(true),
            },
            FieldProperty {
                name: "Caption",
                value: FieldPropertyValue::Text(String::new()),
            },
            FieldProperty {
                name: "DefaultValue",
                value: FieldPropertyValue::Absent,
            },
            FieldProperty {
                name: "ValidationRule",
                value: FieldPropertyValue::Uninspected,
            },
            FieldProperty {
                name: "ValidationText",
                value: FieldPropertyValue::Unsupported,
            },
        ],
    };
    let context = egui::Context::default();
    let render = |events| {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 260.0),
                )),
                events,
                ..Default::default()
            },
            |ui| show(ui, &field),
        );
        output.textures_delta.clear();
        output
    };
    let has_text = |output: &egui::FullOutput, expected: &str| {
        output.shapes.iter().any(|shape| {
            matches!(
                &shape.shape,
                egui::epaint::Shape::Text(text) if text.galley.job.text == expected
            )
        })
    };
    let initial = render(Vec::new());
    assert!(has_text(&initial, "Not stored"));
    assert!(has_text(&initial, "Not inspected"));
    assert!(has_text(&initial, "Unsupported value"));

    let click_text = |output: &egui::FullOutput, label: &str| {
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("painted property control");
        render(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ])
    };
    let selected = click_text(&initial, "Required");
    assert!(has_text(
        &selected,
        "Required is the stored declaration. Reader NULL support does not determine this property."
    ));
    let lookup = click_text(&selected, "Lookup");
    assert!(has_text(
        &lookup,
        "Lookup properties were not inspected. No lookup settings or defaults are inferred."
    ));
    assert!(!has_text(&lookup, "Required"));
    assert!(matches!(
        field.properties[0].value,
        FieldPropertyValue::Boolean(true)
    ));
    assert!(field.reader_nullable.unwrap());
    assert!(
        matches!(&field.properties[1].value, FieldPropertyValue::Text(value) if value.is_empty())
    );
}

#[test]
fn keyboard_copy_uses_full_focused_property_and_refuses_oversize_without_prefix() {
    let context = egui::Context::default();
    let long_literal = format!("{} exact ending", "literal value ".repeat(100));
    let render = |events, value: &str| {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 180.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                for (identity, text) in [("literal", value), ("uninspected", "Not inspected")] {
                    ui.push_id(identity, |ui| {
                        ui.allocate_ui(egui::vec2(250.0, 20.0), |ui| {
                            let response = property_cell(ui, text, true);
                            if let Some(failure) = copy_focused_value(ui, &response, text) {
                                ui.label(failure.key());
                            }
                        });
                    });
                }
            },
        );
        output.textures_delta.clear();
        output
    };
    let tab = || {
        vec![egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    };
    render(Vec::new(), &long_literal);
    render(tab(), &long_literal);
    let copied = render(vec![egui::Event::Copy], &long_literal);
    assert!(copied.platform_output.commands.iter().any(|command| {
        matches!(command, egui::OutputCommand::CopyText(text) if text == &long_literal)
    }));

    render(tab(), &long_literal);
    let uninspected = render(vec![egui::Event::Copy], &long_literal);
    assert!(uninspected.platform_output.commands.iter().any(|command| {
        matches!(command, egui::OutputCommand::CopyText(text) if text == "Not inspected")
    }));
    assert!(!uninspected.platform_output.commands.iter().any(|command| {
        matches!(command, egui::OutputCommand::CopyText(text) if text == &long_literal)
    }));

    render(tab(), &long_literal);
    let oversized = "x".repeat(MAX_PROPERTY_COPY_BYTES + 1);
    let refused = render(vec![egui::Event::Copy], &oversized);
    assert!(
        !refused
            .platform_output
            .commands
            .iter()
            .any(|command| { matches!(command, egui::OutputCommand::CopyText(_)) })
    );
    assert!(refused.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.job.text == "design-copy-limit")
    }));
}

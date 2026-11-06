use super::*;
use crate::Locale;

fn field() -> Field {
    Field {
        name: "Literal table field".into(),
        number: 7,
        native_type: 4,
        storage_bytes: 4,
        fixed: true,
        flags: 0xff,
        extended_flags: 0xff,
        scalar_type: Some("Int32".into()),
        display_type: None,
        field_size: Some("Long Integer".into()),
        reader_nullable: Some(true),
        decimal_parameters: None,
        complex_type_id: None,
        properties: Vec::new(),
    }
}

#[test]
fn reader_carriers_never_invent_declared_access_types_and_unknown_facts_remain_literal() {
    let english = Localizer::default();
    let german = Localizer::new(Locale::De).unwrap();
    let mut field = field();
    let fallback = display(&english, &field);
    assert_eq!(fallback.text, "32-bit integer");
    assert!(fallback.reader_fallback);
    assert_eq!(
        display(&german, &field).text,
        german.text("design-carrier-int32")
    );
    assert_ne!(display(&german, &field).text, fallback.text);

    field.display_type = Some("AutoNumber".into());
    let declared = display(&german, &field);
    assert_eq!(declared.text, german.text("design-type-autonumber"));
    assert!(!declared.reader_fallback);
    assert_eq!(display(&english, &field).text, "AutoNumber");
    assert_eq!(
        field_size(&german, "Long Integer"),
        german.text("design-size-long-integer")
    );
    assert_eq!(field_size(&german, "120"), "120");
    assert_eq!(field_size(&german, "Vendor size  19"), "Vendor size  19");

    field.display_type = Some("Vendor type:  Custom_Ä".into());
    assert_eq!(display(&german, &field).text, "Vendor type:  Custom_Ä");
    field.display_type = None;
    field.scalar_type = Some("VendorCarrier<X>".into());
    assert_eq!(display(&german, &field).text, "VendorCarrier<X>");
    assert_eq!(field.name, "Literal table field");
    assert_eq!(field.field_size.as_deref(), Some("Long Integer"));
    assert_eq!(carrier(&english, "Text"), "Unicode text");
    assert_eq!(carrier(&english, "Currency"), "Fixed-scale currency");
}

#[test]
fn german_design_localizes_only_type_labels_and_retains_literal_metadata_and_diagnostics() {
    use crate::preview::{Preview, Scene};
    use crate::{FieldProperty, FieldPropertyValue, State};
    use egui_kittest::{Harness, kittest::Queryable};

    let mut preview = Preview::new(Scene::Design);
    let mut state = std::mem::take(preview.state_mut());
    state.localizer = Localizer::new(Locale::De).unwrap();
    let type_label = state.localizer.text("design-type-short-text");
    let native_details = state.localizer.text("design-native-details");
    let mut declaration = state.schema.take().unwrap();
    let field = &mut declaration.fields[0];
    field.name = "Currency".into();
    field.display_type = Some("Short Text".into());
    field.scalar_type = Some("Int32".into());
    field.field_size = Some("120".into());
    field.properties = vec![
        FieldProperty {
            name: "Caption",
            value: FieldPropertyValue::Text("AutoNumber".into()),
        },
        FieldProperty {
            name: "Description",
            value: FieldPropertyValue::Text("Currency".into()),
        },
        FieldProperty {
            name: "DefaultValue",
            value: FieldPropertyValue::Text("AutoNumber".into()),
        },
    ];
    state.set_schema(declaration);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .build_ui_state(
            |ui, state: &mut State| assert!(state.show(ui).is_empty()),
            state,
        );
    crate::configure(&harness.ctx);
    harness.run();

    assert!(harness.get_all_by_label(&type_label).count() > 0);
    assert!(harness.query_by_label("Short Text").is_none());
    assert!(harness.get_all_by_label("Currency").count() >= 2);
    assert!(harness.get_all_by_label("AutoNumber").count() >= 2);
    assert!(harness.query_by_label("120").is_some());
    harness.get_by_label(&native_details).click();
    harness.run();
    assert!(harness.query_by_label("Int32").is_some());
    assert_eq!(
        harness.state().schema.as_ref().unwrap().fields[0].name,
        "Currency"
    );
    assert_eq!(harness.state().localizer.diagnostics(), Default::default());
}

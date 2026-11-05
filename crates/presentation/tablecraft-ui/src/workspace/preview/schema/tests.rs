use super::*;
use crate::preview::Scene;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn synthetic_design_scenarios_keep_missing_and_uninspected_observations_distinct() {
    let preview = Preview::new(Scene::DesignUninspected);
    let second_name = preview.state().schema.as_ref().unwrap().fields[1]
        .name
        .clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 680.0))
        .build_ui_state(|ui, preview: &mut Preview| preview.show(ui), preview);
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label("No property observations supplied.")
            .is_some()
    );
    let original = harness.state().state().data.as_ref().unwrap().rows.as_ptr();

    harness.get_by_label(&second_name).click_accesskit();
    harness.run_steps(3);
    for value in ["Not inspected", "Not stored", "Unsupported value"] {
        assert!(
            harness.get_all_by_label(value).next().is_some(),
            "missing observation: {value}"
        );
    }
    assert!(
        harness
            .query_by_label("No property observations supplied.")
            .is_none()
    );
    assert_eq!(harness.state().state().selected_field, 1);
    assert_eq!(
        harness.state().state().data.as_ref().unwrap().rows.as_ptr(),
        original
    );
    assert!(!harness.state().state().busy);

    let preview = Preview::new(Scene::DesignLong);
    let schema = preview.state().schema.as_ref().unwrap();
    assert!(schema.fields.len() > 64);
    for (number, field) in schema.fields.iter().enumerate() {
        assert_eq!(field.number, number);
        assert!(field.name.len() <= 64);
        assert!(field.name.contains("deliberately_long_name"));
    }
    let FieldPropertyValue::Text(description) = &schema.fields[0].properties[0].value else {
        panic!("long design must retain its full synthetic literal observation");
    };
    assert!(description.len() > 500);
    assert!(preview.state().mock_preview);
    assert_eq!(preview.state().data.as_ref().unwrap().table, "Customers");
}

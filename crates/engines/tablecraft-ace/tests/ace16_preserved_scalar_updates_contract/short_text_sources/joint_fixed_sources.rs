//! Read-only qualification of complete joint Text/fixed source images.
#[path = "joint_fixed_sources/model.rs"]
pub(crate) mod model;

use super::super::joint_fixed_sources::model as fixed_model;
use super::super::{SCOPE, budget};
use super::{model as text_model, zero_length_source};
use std::io::Cursor;
use tablecraft_ace::{
    PropertyGroupKind, inspect_table_properties, inspect_table_schema, read_table_projection,
};
use tablecraft_core::values::Value;

fn qualify(bytes: &[u8], case: &model::Case) {
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 16);
    let names = schema
        .columns()
        .iter()
        .filter(|field| field.scalar_type().is_some())
        .map(|field| field.name())
        .collect::<Vec<_>>();
    let projected = read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &names,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.rows().len(), 3);
    for row in 0..3 {
        let selected = projected
            .columns()
            .iter()
            .position(|field| field.name() == "Short_Text")
            .unwrap();
        assert_eq!(
            projected.rows()[row][selected],
            model::read_text(bytes, row).map_or(Value::Null, Value::Text)
        );
        for column in fixed_model::columns(case.flavor) {
            let selected = projected
                .columns()
                .iter()
                .position(|field| field.name() == column.name())
                .unwrap();
            assert_eq!(
                projected.rows()[row][selected],
                fixed_model::read(bytes, column, row).value()
            );
        }
    }
}

#[test]
fn qualify_joint_text_fixed_originals_and_expected_before_writer() {
    let mut views = 0;
    let mut expected_images = 0;
    let mut refusals = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            model::assert_protected(&case);
            qualify(&case.before, &case);
            views += 1;
            if let Some(expected) = &case.expected {
                qualify(expected, &case);
                expected_images += 1;
                views += 1;
            } else {
                assert!(case.refusal.is_some());
                refusals += 1;
            }
            if matches!(
                recipe,
                model::Recipe::TextRequired | model::Recipe::DisallowEmptyText
            ) {
                let properties = inspect_table_properties(
                    &mut Cursor::new(&case.before),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let group = properties
                    .group(PropertyGroupKind::Column, "Short_Text")
                    .unwrap();
                if recipe == model::Recipe::TextRequired {
                    assert_eq!(group.get("Required").unwrap().boolean(), Ok(true));
                    for row in 0..3 {
                        assert!(model::read_text(&case.before, row).is_some());
                    }
                    assert_eq!(model::read_text(&case.before, 1).as_deref(), Some("é"));
                } else {
                    assert_eq!(group.get("AllowZeroLength").unwrap().boolean(), Ok(false));
                    for row in 0..3 {
                        assert_ne!(model::read_text(&case.before, row).as_deref(), Some(""));
                    }
                }
            }
        }
    }
    assert_eq!((views, expected_images, refusals), (90, 36, 18));
}

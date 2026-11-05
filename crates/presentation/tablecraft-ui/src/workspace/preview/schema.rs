//! Bounded synthetic metadata variants for Table Design presentation review.
use super::{Preview, fixtures};
use crate::{FieldProperty, FieldPropertyValue};

const REPEATED_FIELD_GROUPS: usize = 6;

impl Preview {
    pub(super) fn prepare_long_design(&mut self) {
        let mut schema = fixtures::schema("Customers");
        schema.fields.clear();
        for _ in 0..REPEATED_FIELD_GROUPS {
            for mut field in fixtures::schema("Customers").fields {
                field.number = schema.fields.len();
                field.name = format!(
                    "Inspection_field_with_a_deliberately_long_name_{:03}",
                    field.number
                );
                schema.fields.push(field);
            }
        }

        let field = &mut schema.fields[0];
        field.properties = vec![
            FieldProperty {
                name: "Description",
                value: FieldPropertyValue::Text(
                    "Synthetic presentation observation. ".repeat(24),
                ),
            },
            FieldProperty {
                name: "Caption",
                value: FieldPropertyValue::Text(
                    "Long synthetic caption retained as literal metadata without changing the field name".into(),
                ),
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
        ];
        self.state.prepare_schema_view();
        self.state.set_schema(schema);
    }

    pub(super) fn prepare_uninspected_design(&mut self) {
        let mut schema = fixtures::schema("Customers");
        let field = &mut schema.fields[0];
        field.field_size = None;
        field.properties.clear();

        let field = &mut schema.fields[1];
        field.properties = vec![
            FieldProperty {
                name: "Description",
                value: FieldPropertyValue::Uninspected,
            },
            FieldProperty {
                name: "Caption",
                value: FieldPropertyValue::Absent,
            },
            FieldProperty {
                name: "ValidationRule",
                value: FieldPropertyValue::Unsupported,
            },
            FieldProperty {
                name: "Required",
                value: FieldPropertyValue::Boolean(false),
            },
        ];
        self.state.prepare_schema_view();
        self.state.set_schema(schema);
    }
}

#[cfg(test)]
mod tests;

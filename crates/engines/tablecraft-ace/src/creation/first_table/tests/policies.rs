use super::{budget, columns, fixtures, image};
use crate::{CreateDatabaseError, FirstTableDefinition};
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation,
    canonical_creation_properties::{Policy, with_policies},
};

#[test]
fn exact_optional_field_policies_match_independent_whole_native_images() {
    for required in [None, Some(false), Some(true)] {
        for allow_zero_length in [None, Some(false), Some(true)] {
            if required.is_none() && allow_zero_length.is_none() {
                continue;
            }
            let source = fixtures::source(fixtures::Case::ShortText);
            let mut declarations = columns(source.columns);
            if let Some(required) = required {
                declarations[1] = declarations[1].with_required(required);
            }
            if let Some(allow) = allow_zero_length {
                declarations[1] = declarations[1].with_allow_zero_length(allow).unwrap();
            }
            let definition =
                FirstTableDefinition::capture("Items", &declarations, &mut budget(), &|| false)
                    .unwrap();
            let retained = definition
                .borrowed_columns(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(retained[0].required(), None);
            assert_eq!(retained[0].allow_zero_length(), None);
            assert_eq!(retained[1].required(), required);
            assert_eq!(retained[1].allow_zero_length(), allow_zero_length);
            drop(retained);

            let expected = with_policies(
                source.bytes,
                &[Policy {
                    column: "Name",
                    required,
                    allow_zero_length,
                }],
            );
            let plan = image(&canonical_blank_creation::source())
                .plan_first_table(definition, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.after_bytes(), expected);
        }
    }
}

#[test]
fn generated_primary_policy_cannot_be_reintroduced_after_autonumber_declaration() {
    let source = fixtures::source(fixtures::Case::ShortText);
    for required in [false, true] {
        let mut declarations = columns(source.columns);
        declarations[0] = declarations[0].with_required(required);
        assert!(matches!(
            FirstTableDefinition::capture("Items", &declarations, &mut budget(), &|| false),
            Err(CreateDatabaseError::Declaration)
        ));
    }
}

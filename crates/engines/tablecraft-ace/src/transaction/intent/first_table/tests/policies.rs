use super::{assert_intent_resources, budget, literal};
use crate::{
    CreateColumn, FirstTableDefinition, FirstTableIntentError, NativeFirstTableIntent, NativeImage,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn policy_wire(default: bool) -> Vec<u8> {
    let mut raw = literal();
    raw[..8].copy_from_slice(b"TCNT0003");
    raw[12..14].copy_from_slice(&3_u16.to_le_bytes());
    raw[60..62].copy_from_slice(&2_u16.to_le_bytes());
    // Required=true, AllowZeroLength=true, optional empty Text default.
    raw.extend([4, 0, 11, if default { 31 } else { 30 }, 255, 0, 0, 0]);
    raw.extend(b"Name");
    if default {
        raw.extend([6, 0, 0, 0, 0, 0, 0, 0]);
    }
    let total = raw.len() as u32;
    raw[8..12].copy_from_slice(&total.to_le_bytes());
    raw
}

#[test]
fn policy_v3_matches_independent_wire_and_restores_option_values_and_defaults() {
    for default in [false, true] {
        let empty = Value::Text(String::new());
        let primary = CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap();
        let mut name = CreateColumn::short_text("Name", 255)
            .unwrap()
            .with_required(true)
            .with_allow_zero_length(true)
            .unwrap();
        if default {
            name = name.with_default_literal(&empty).unwrap();
        }
        let definition =
            FirstTableDefinition::capture("Items", &[primary, name], &mut budget(), &|| false)
                .unwrap();
        let plan = NativeImage::capture(
            &mut Cursor::new(canonical_blank_creation::source()),
            CatalogScope::new(490),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
        let raw = policy_wire(default);
        assert_eq!(plan.encode_intent(&mut budget(), &|| false).unwrap(), raw);
        let decoded = NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        let retained = decoded
            .definition
            .borrowed_columns(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(retained[1].required(), Some(true));
        assert_eq!(retained[1].allow_zero_length(), Some(true));
        assert_eq!(retained[1].default_literal(), default.then_some(&empty));
        assert_intent_resources(plan, raw);
    }
    // Presence of explicit false is preserved, rather than elided to None.
    let mut raw = policy_wire(false);
    raw[91] = 10;
    let decoded = NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    let retained = decoded
        .definition
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(retained[1].required(), Some(false));
    assert_eq!(retained[1].allow_zero_length(), Some(false));
}

#[test]
fn policy_v3_refuses_noncanonical_flags_primary_policies_wrong_carriers_and_downgrades() {
    let original = policy_wire(false);
    for (offset, value) in [
        (12, 2),
        (81, 2),
        (91, 0),
        (91, 4),
        (91, 16),
        (91, 32),
        (90, 4),
    ] {
        let mut raw = original.clone();
        raw[offset] = value;
        assert!(
            NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
    for version in *b"12" {
        let mut raw = original.clone();
        raw[7] = version;
        raw[12] = if version == b'1' { 1 } else { 2 };
        assert!(NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).is_err());
    }
    for length in 0..original.len() {
        assert!(
            NativeFirstTableIntent::decode(&original[..length], &mut budget(), &|| false).is_err()
        );
    }
    let mut unknown = original;
    unknown[7] = b'5';
    assert!(matches!(
        NativeFirstTableIntent::decode(&unknown, &mut budget(), &|| false),
        Err(FirstTableIntentError::UnsupportedVersion)
    ));
}

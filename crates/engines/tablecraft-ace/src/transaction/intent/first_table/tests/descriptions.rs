//! TCNT5 retains exact optional text while legacy record languages stay closed.
use super::{budget, literal};
use crate::{CreateColumn, FirstTableDefinition, FirstTableIntentError, NativeFirstTableIntent};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn definition(primary: Option<&str>, ordinary: Option<&str>) -> FirstTableDefinition {
    let mut id = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    if let Some(text) = primary {
        id = id.with_description(text).unwrap();
    }
    let mut columns = vec![id];
    if let Some(text) = ordinary {
        columns.push(
            CreateColumn::short_text("Title", 20)
                .unwrap()
                .with_description(text)
                .unwrap(),
        );
    }
    FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap()
}

fn wire(primary: Option<&str>, ordinary: Option<&str>) -> Vec<u8> {
    let mut raw = literal();
    raw[..8].copy_from_slice(b"TCNT0005");
    raw[12..14].copy_from_slice(&5_u16.to_le_bytes());
    if let Some(text) = primary {
        raw[81] = 32;
        raw.extend((text.len() as u16).to_le_bytes());
        raw.extend(text.as_bytes());
    }
    if let Some(text) = ordinary {
        raw[60..62].copy_from_slice(&2_u16.to_le_bytes());
        raw.extend([5, 0, 11, 32, 20, 0, 0, 0]);
        raw.extend(b"Title");
        raw.extend((text.len() as u16).to_le_bytes());
        raw.extend(text.as_bytes());
    }
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

#[test]
fn described_schema_matches_independent_bytes_and_keeps_absence_and_empty_exact() {
    let boundary = "🗃".repeat(127) + "x";
    for text in ["", "Résumé", "quotes \"\" and # / 🗃", boundary.as_str()] {
        for primary in [false, true] {
            let (id, ordinary) = if primary {
                (Some(text), None)
            } else {
                (None, Some(text))
            };
            let expected = wire(id, ordinary);
            let definition = definition(id, ordinary);
            let raw =
                super::super::encode_declaration_template(&definition, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(raw, expected);
            let decoded =
                NativeFirstTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
            let columns = decoded
                .definition
                .borrowed_columns(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(columns[0].description(), id);
            if !primary {
                assert_eq!(columns[1].description(), ordinary);
            }
            assert_eq!(
                super::super::encode_declaration_template(
                    &decoded.definition,
                    &mut budget(),
                    &|| false
                )
                .unwrap(),
                expected
            );
        }
    }
    let absent =
        super::super::encode_declaration_template(&definition(None, None), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(absent, literal());
}

#[test]
fn described_schemas_compose_all_existing_default_and_policy_families() {
    for value in [Value::Int32(17), Value::Byte(255), Value::Null] {
        let kind = if matches!(value, Value::Byte(_)) {
            ScalarType::Byte
        } else {
            ScalarType::Int32
        };
        let columns = [
            CreateColumn::new("Id", ScalarType::Int32)
                .with_primary_key("PrimaryId")
                .unwrap()
                .with_incrementing_autonumber()
                .unwrap()
                .with_description("")
                .unwrap(),
            CreateColumn::new("Number", kind)
                .with_required(false)
                .with_default_literal(&value)
                .unwrap()
                .with_description("plain text, not an expression")
                .unwrap(),
        ];
        let definition =
            FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
        let raw = super::super::encode_declaration_template(&definition, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(&raw[..8], b"TCNT0005");
        let decoded = NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        let actual = decoded
            .definition
            .borrowed_columns(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(actual[1].default_literal(), Some(&value));
        assert_eq!(actual[1].required(), Some(false));
        assert_eq!(actual[0].description(), Some(""));
    }
}

#[test]
fn description_framing_and_utf16_bounds_refuse_before_decoded_ownership() {
    let mut cases = vec![
        wire(None, None),
        wire(Some(&"x".repeat(256)), None),
        wire(Some(&"🗃".repeat(128)), None),
    ];
    let mut missing_length = wire(Some(""), None);
    missing_length.truncate(88);
    cases.push(missing_length);
    let mut missing_payload = wire(Some(""), None);
    missing_payload[88..90].copy_from_slice(&1_u16.to_le_bytes());
    cases.push(missing_payload);
    let mut invalid_utf8 = wire(Some("x"), None);
    invalid_utf8[90] = 255;
    cases.push(invalid_utf8);
    let mut unexpected_empty = wire(Some(""), None);
    unexpected_empty[81] = 0;
    cases.push(unexpected_empty);
    let mut wrong_profile = wire(Some(""), None);
    wrong_profile[12] = 4;
    cases.push(wrong_profile);
    for mut raw in cases {
        let length = raw.len() as u32;
        raw[8..12].copy_from_slice(&length.to_le_bytes());
        assert!(matches!(
            NativeFirstTableIntent::decode(
                &raw,
                &mut ResourceBudget::new(0, 0, 1_000_000),
                &|| false
            ),
            Err(FirstTableIntentError::InvalidRecord | FirstTableIntentError::UnsupportedProfile)
        ));
    }
    for version in 1..=4 {
        let mut raw = wire(Some(""), None);
        raw[7] = b'0' + version;
        raw[12] = version;
        assert!(matches!(
            NativeFirstTableIntent::decode(
                &raw,
                &mut ResourceBudget::new(0, 0, 1_000_000),
                &|| false
            ),
            Err(FirstTableIntentError::InvalidRecord)
        ));
    }
    // The old ceiling remains exact even though TCNT5 admits longer records.
    let mut legacy_oversized = vec![0; 21_249];
    legacy_oversized[..8].copy_from_slice(b"TCNT0004");
    // Preserve the historical one-step entry cancellation check.
    let mut entry_only = ResourceBudget::new(0, 0, 1);
    assert!(matches!(
        NativeFirstTableIntent::decode(&legacy_oversized, &mut entry_only, &|| false),
        Err(FirstTableIntentError::InvalidRecord)
    ));
    assert_eq!(entry_only.remaining(Resource::Steps), 0);
    assert_eq!(entry_only.remaining(Resource::Bytes), 0);
}

#[test]
fn described_schema_encoding_and_decoding_pay_exact_pools_and_poll_cancellation() {
    let definition = definition(Some("Résumé 🗃"), Some(""));
    let image = crate::NativeImage::capture(
        &mut Cursor::new(canonical_blank_creation::source()),
        CatalogScope::new(487),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_first_table(definition, &mut budget(), &|| false)
        .unwrap();
    super::assert_intent_resources(plan, wire(Some("Résumé 🗃"), Some("")));
}

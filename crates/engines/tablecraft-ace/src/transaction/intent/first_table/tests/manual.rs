//! Literal TCNT6 manual-key and keyless declarations retain real field positions.
use super::{assert_intent_resources, budget, literal};
use crate::{
    CreateColumn, CreateDatabaseError, FirstTableDefinition, FirstTableIntentError,
    NativeFirstTableIntent, NativeImage,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn definition(key: Option<ScalarType>) -> FirstTableDefinition {
    let name = CreateColumn::short_text("Name", 20).unwrap();
    let mut score = CreateColumn::new("Score", key.unwrap_or(ScalarType::Int32));
    if key.is_some() {
        score = score.with_primary_key("PrimaryScore").unwrap();
    }
    FirstTableDefinition::capture_manual("Items", &[name, score], &mut budget(), &|| false).unwrap()
}

fn wire(tag: Option<u8>) -> Vec<u8> {
    let mut raw = literal()[..64].to_vec();
    raw[..8].copy_from_slice(b"TCNT0006");
    raw[12..14].copy_from_slice(&6_u16.to_le_bytes());
    raw[58..60].copy_from_slice(&(if tag.is_some() { 12_u16 } else { 0 }).to_le_bytes());
    raw[60..62].copy_from_slice(&2_u16.to_le_bytes());
    raw[62..64].copy_from_slice(&(if tag.is_some() { 1_u16 } else { 65535 }).to_le_bytes());
    raw.extend(b"Items");
    if tag.is_some() {
        raw.extend(b"PrimaryScore");
    }
    raw.extend([4, 0, 11, 0, 20, 0, 0, 0]);
    raw.extend(b"Name");
    raw.extend([5, 0, tag.unwrap_or(4), 0, 0, 0, 0, 0]);
    raw.extend(b"Score");
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

#[test]
fn manual_keyless_and_nonfirst_numeric_keys_match_independent_wire_without_generated_id() {
    for (kind, tag) in [
        (None, None),
        (Some(ScalarType::Int32), Some(4)),
        (Some(ScalarType::Int64), Some(5)),
        (Some(ScalarType::Currency), Some(6)),
    ] {
        let definition = definition(kind);
        let expected = wire(tag);
        let actual =
            super::super::encode_declaration_template(&definition, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(actual, expected);
        let decoded = NativeFirstTableIntent::decode(&expected, &mut budget(), &|| false).unwrap();
        assert!(decoded.definition.is_manual());
        assert_eq!(decoded.definition.primary_position(), tag.map(|_| 1));
        assert_eq!(
            decoded.definition.primary_name(),
            tag.map(|_| "PrimaryScore")
        );
        let columns = decoded
            .definition
            .borrowed_columns(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(columns.len(), 2);
        assert_eq!(columns[0].name(), "Name");
        assert_eq!(columns[0].primary_key_name(), None);
        assert!(
            columns
                .iter()
                .all(|column| !column.incrementing_autonumber())
        );
        assert_eq!(
            super::super::encode_declaration_template(&decoded.definition, &mut budget(), &|| {
                false
            })
            .unwrap(),
            expected
        );
        decoded
            .verify_before(&canonical_blank_creation::source(), &mut budget(), &|| {
                false
            })
            .unwrap();
    }
}

#[test]
fn manual_profile_retains_optional_descriptions_defaults_policies_and_position_zero() {
    let zero = Value::Int32(0);
    let columns = [
        CreateColumn::new("Score", ScalarType::Int32)
            .with_primary_key("ActualKey")
            .unwrap()
            .with_default_literal(&zero)
            .unwrap()
            .with_description("")
            .unwrap(),
        CreateColumn::short_text("Name", 20)
            .unwrap()
            .with_required(false)
            .with_allow_zero_length(true)
            .unwrap()
            .with_description("Résumé 🗃")
            .unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture_manual("Items", &columns, &mut budget(), &|| false).unwrap();
    let raw =
        super::super::encode_declaration_template(&definition, &mut budget(), &|| false).unwrap();
    assert_eq!(&raw[..8], b"TCNT0006");
    assert_eq!(&raw[62..64], &[0, 0]);
    let decoded = NativeFirstTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert!(decoded.definition.is_manual());
    assert_eq!(decoded.definition.primary_position(), Some(0));
    let actual = decoded
        .definition
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(actual[0].default_literal(), Some(&zero));
    assert_eq!(actual[0].description(), Some(""));
    assert!(!actual[0].incrementing_autonumber());
    assert_eq!(actual[1].description(), Some("Résumé 🗃"));
    assert_eq!(actual[1].required(), Some(false));
    assert_eq!(actual[1].allow_zero_length(), Some(true));
}

#[test]
fn malformed_manual_key_headers_and_generated_tags_refuse_before_owned_decode() {
    for case in 0..8 {
        let mut raw = wire(Some(4));
        match case {
            0 => raw[12] = 5,
            1 => raw[62..64].copy_from_slice(&65535_u16.to_le_bytes()),
            2 => raw[62..64].copy_from_slice(&2_u16.to_le_bytes()),
            3 => raw[58..60].copy_from_slice(&0_u16.to_le_bytes()),
            4 => raw[62..64].copy_from_slice(&0_u16.to_le_bytes()),
            5 => raw[81 + 2] = 0,
            6 => raw[93 + 2] = 0,
            7 => raw.push(0),
            _ => unreachable!(),
        }
        let mut no_ownership = ResourceBudget::new(0, 0, 1_000_000);
        assert!(
            matches!(
                NativeFirstTableIntent::decode(&raw, &mut no_ownership, &|| false),
                Err(FirstTableIntentError::InvalidRecord
                    | FirstTableIntentError::UnsupportedProfile
                    | FirstTableIntentError::Create(CreateDatabaseError::Declaration))
            ),
            "case {case}"
        );
        assert_eq!(no_ownership.remaining(Resource::Bytes), 0);
    }
    for version in 1..=5 {
        let mut raw = wire(None);
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
    let mut no_key_with_name = wire(None);
    no_key_with_name[58..60].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        NativeFirstTableIntent::decode(
            &no_key_with_name,
            &mut ResourceBudget::new(0, 0, 1_000_000),
            &|| false
        ),
        Err(FirstTableIntentError::InvalidRecord)
    ));
}

#[test]
fn manual_schema_retains_canonical_binding_and_exact_resource_cancellation_contract() {
    for key in [false, true] {
        let definition = definition(key.then_some(ScalarType::Int32));
        let image = NativeImage::capture(
            &mut Cursor::new(canonical_blank_creation::source()),
            CatalogScope::new(488),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_first_table(definition, &mut budget(), &|| false)
            .unwrap();
        let raw = wire(key.then_some(4));
        assert_intent_resources(plan, raw.clone());
        let mut changed = raw;
        changed[24] ^= 1;
        assert!(matches!(
            NativeFirstTableIntent::decode(&changed, &mut budget(), &|| false),
            Err(FirstTableIntentError::BeforeMismatch)
        ));
    }
}

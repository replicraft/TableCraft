//! Native declarations remain a distinct original request, including at small counts.
mod recovery;
use super::{FirstTableIntentError, NativeFirstTableIntent, budget, literal};
use crate::{CreateColumn, FirstTableDefinition};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn header(table: &str, primary: &str, count: u16, flags: u16, position: u16) -> Vec<u8> {
    let before = canonical_blank_creation::source();
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update(28_672_u64.to_le_bytes());
    hash.update(before);
    let mut bytes = b"TCNT0007".to_vec();
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(7_u16.to_le_bytes());
    bytes.extend(flags.to_le_bytes());
    bytes.extend(28_672_u64.to_le_bytes());
    bytes.extend(hash.finalize());
    for value in [table.len() as u16, primary.len() as u16, count, position] {
        bytes.extend(value.to_le_bytes());
    }
    assert_eq!(bytes.len(), 64);
    bytes.extend(table.as_bytes());
    bytes.extend(primary.as_bytes());
    bytes
}

fn field(bytes: &mut Vec<u8>, name: &str, tag: u8) {
    bytes.extend((name.len() as u16).to_le_bytes());
    bytes.extend([tag, 0, 0, 0, 0, 0]);
    bytes.extend(name.as_bytes());
}

fn finish(bytes: &mut [u8]) {
    let length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&length.to_le_bytes());
}

#[test]
fn native_255_field_request_matches_independent_wire_and_keeps_original_decoder_closed() {
    let names: Vec<_> = (1..255).map(|index| format!("Field{index:03}")).collect();
    let mut columns = vec![
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
    ];
    columns.extend(
        names
            .iter()
            .map(|name| CreateColumn::new(name, ScalarType::Int32)),
    );
    let definition =
        FirstTableDefinition::capture_native("Native", &columns, &mut budget(), &|| false).unwrap();
    let encoded =
        super::super::codec::encode_template(&definition, &mut budget(), &|| false).unwrap();
    let mut expected = header("Native", "PrimaryId", 255, 0, 0);
    field(&mut expected, "Id", 0);
    for name in &names {
        field(&mut expected, name, 4);
    }
    finish(&mut expected);
    assert_eq!(encoded, expected);
    let decoded =
        NativeFirstTableIntent::decode_native(&expected, &mut budget(), &|| false).unwrap();
    assert!(decoded.is_native());
    assert_eq!(decoded.column_count(), 255);
    assert_eq!(decoded.table(), "Native");
    assert_eq!(
        super::super::codec::encode_template(&decoded.definition, &mut budget(), &|| false)
            .unwrap(),
        expected
    );
    assert!(matches!(
        NativeFirstTableIntent::decode(&expected, &mut budget(), &|| false),
        Err(FirstTableIntentError::UnsupportedVersion)
    ));
    assert!(matches!(
        NativeFirstTableIntent::decode_native(&literal(), &mut budget(), &|| false),
        Err(FirstTableIntentError::UnsupportedVersion)
    ));
}

#[test]
fn native_manual_primary_and_keyless_requests_preserve_exact_original_positions() {
    for position in [Some(1), None] {
        let second = CreateColumn::new("Key", ScalarType::Int32);
        let second = match position {
            Some(_) => second.with_primary_key("PK").unwrap(),
            None => second,
        };
        let columns = [CreateColumn::new("Value", ScalarType::Int32), second];
        let definition =
            FirstTableDefinition::capture_manual_native("Manual", &columns, &mut budget(), &|| {
                false
            })
            .unwrap();
        let mut expected = header(
            "Manual",
            if position.is_some() { "PK" } else { "" },
            2,
            1,
            position.unwrap_or(u16::MAX),
        );
        field(&mut expected, "Value", 4);
        field(&mut expected, "Key", 4);
        finish(&mut expected);
        assert_eq!(
            super::super::codec::encode_template(&definition, &mut budget(), &|| false).unwrap(),
            expected
        );
        let decoded =
            NativeFirstTableIntent::decode_native(&expected, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.definition.primary_position(),
            position.map(usize::from)
        );
        assert!(decoded.definition.is_manual());
    }
}

#[test]
fn native_malformed_headers_refuse_before_declaration_allocation() {
    let mut original = header("Native", "PrimaryId", 1, 0, 0);
    field(&mut original, "Id", 0);
    finish(&mut original);
    for (offset, value) in [(14, 2), (60, 0), (61, 1), (62, 1)] {
        let mut malformed = original.clone();
        malformed[offset] = value;
        let mut allowance = ResourceBudget::new(0, 1000, 100_000);
        assert!(matches!(
            NativeFirstTableIntent::decode_native(&malformed, &mut allowance, &|| false),
            Err(FirstTableIntentError::InvalidRecord)
        ));
    }
    assert!(NativeFirstTableIntent::decode_native(&original, &mut budget(), &|| true).is_err());
    assert!(
        NativeFirstTableIntent::decode_native(
            &original,
            &mut ResourceBudget::new(0, 1000, 100_000),
            &|| false
        )
        .is_err()
    );
}

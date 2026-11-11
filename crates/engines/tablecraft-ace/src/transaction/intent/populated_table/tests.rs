//! Independent outer wire literals and small malformed-prefix checks, no giant allocations.
use super::*;
use sha2::{Digest, Sha256};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn declaration() -> FirstTableDefinition {
    FirstTableDefinition::capture_manual_native(
        "Archive",
        &[crate::CreateColumn::new("CopiedScore", ScalarType::Int32)],
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn populated_outer_wire_has_literal_u64_envelope_and_complete_unomitted_values() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let definition = declaration();
    // Only the already independently qualified nested declaration codec is shared.
    // The new outer header and every row carrier are written literally below.
    let nested =
        super::super::first_table::encode_declaration_template(&definition, &mut budget(), &|| {
            false
        })
        .unwrap();
    let rows = vec![vec![Value::Int32(123)], vec![Value::Null]];
    let intent =
        NativePopulatedAddTableIntent::new(definition, rows, &before, &mut budget(), &|| false)
            .unwrap();
    let encoded = intent.encode(&mut budget(), &|| false).unwrap();
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(&before);
    let binding: [u8; 32] = hash.finalize().into();
    let mut expected = b"TCPT0001".to_vec();
    expected.extend((80_u64 + nested.len() as u64 + 20).to_le_bytes());
    expected.extend(1_u16.to_le_bytes());
    expected.extend([0; 6]);
    expected.extend((before.len() as u64).to_le_bytes());
    expected.extend(binding);
    expected.extend((nested.len() as u32).to_le_bytes());
    expected.extend(2_u32.to_le_bytes());
    expected.extend(1_u16.to_le_bytes());
    expected.extend([0; 6]);
    expected.extend(nested);
    expected.extend([2, 0, 0, 0, 4, 0, 0, 0, 123, 0, 0, 0]);
    expected.extend([0; 8]);
    assert_eq!(encoded.len(), expected.len());
    assert_eq!(
        encoded.iter().zip(&expected).position(|(a, b)| a != b),
        None
    );
    let decoded =
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.table(), "Archive");
    assert_eq!(decoded.rows, &[vec![Value::Int32(123)], vec![Value::Null]]);
}

#[test]
fn oversized_totals_counts_and_late_invalid_carriers_refuse_before_decoded_allocations() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let encoded = NativePopulatedAddTableIntent::new(
        declaration(),
        vec![vec![Value::Int32(1)], vec![Value::Null]],
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .encode(&mut budget(), &|| false)
    .unwrap();
    for total in [u64::from(u32::MAX) + 1, u64::MAX] {
        let mut malformed = encoded.clone();
        malformed[8..16].copy_from_slice(&total.to_le_bytes());
        assert!(matches!(
            NativePopulatedAddTableIntent::decode(
                &malformed,
                &mut ResourceBudget::new(0, u64::MAX, u64::MAX),
                &|| false
            ),
            Err(PopulatedTableIntentError::InvalidRecord)
        ));
    }
    for offset in [64, 68] {
        let mut malformed = encoded.clone();
        malformed[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            NativePopulatedAddTableIntent::decode(
                &malformed,
                &mut ResourceBudget::new(0, u64::MAX, u64::MAX),
                &|| false
            ),
            Err(PopulatedTableIntentError::InvalidRecord)
        ));
    }
    let mut malformed = encoded;
    let last = malformed.len() - 8;
    malformed[last] = 255;
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(
            &malformed,
            &mut ResourceBudget::new(0, u64::MAX, u64::MAX),
            &|| false
        ),
        Err(PopulatedTableIntentError::Binding(
            NativeIntentError::InvalidRecord
        ))
    ));
}

#[test]
fn nested_inherited_properties_boolean_null_and_wrong_scalar_are_not_coerced() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let column = crate::CreateColumn::new("CopiedScore", ScalarType::Int32).with_required(false);
    let inherited =
        FirstTableDefinition::capture_manual_native("Archive", &[column], &mut budget(), &|| false)
            .unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::new(inherited, Vec::new(), &before, &mut budget(), &|| {
            false
        }),
        Err(PopulatedTableIntentError::UnsupportedProfile)
    ));
    let boolean = FirstTableDefinition::capture_manual_native(
        "Archive",
        &[crate::CreateColumn::new("Enabled", ScalarType::Boolean)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::new(
            boolean,
            vec![vec![Value::Null]],
            &before,
            &mut budget(),
            &|| false
        ),
        Err(PopulatedTableIntentError::InvalidRecord)
    ));
    assert!(matches!(
        NativePopulatedAddTableIntent::new(
            declaration(),
            vec![vec![Value::Int64(123)]],
            &before,
            &mut budget(),
            &|| false
        ),
        Err(PopulatedTableIntentError::InvalidRecord)
    ));
}

#[test]
fn original_empty_table_decoders_and_new_populated_decoder_remain_closed() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let populated = NativePopulatedAddTableIntent::new(
        declaration(),
        Vec::new(),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .encode(&mut budget(), &|| false)
    .unwrap();
    assert!(
        crate::NativeAddTableIntent::decode_native(&populated, &mut budget(), &|| false).is_err()
    );
    assert!(
        crate::NativeFirstTableIntent::decode_native(&populated, &mut budget(), &|| false).is_err()
    );
    let empty = crate::NativeAddTableIntent::new_native(
        declaration(),
        crate::AddTableSourceProfile::Retained,
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .encode(&mut budget(), &|| false)
    .unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&empty, &mut budget(), &|| false),
        Err(PopulatedTableIntentError::InvalidRecord)
    ));
    let mut unknown = populated;
    unknown[7] = b'2';
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&unknown, &mut budget(), &|| false),
        Err(PopulatedTableIntentError::UnsupportedVersion)
    ));
}

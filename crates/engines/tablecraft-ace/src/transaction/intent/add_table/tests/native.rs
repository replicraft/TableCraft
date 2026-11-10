//! TCAT7 binds native request language and source cohort independently.
mod recovery;
use super::{budget, template};
use crate::{
    AddTableIntentError, AddTableSourceProfile, CreateColumn, FirstTableDefinition,
    NativeAddTableIntent,
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

fn definition() -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 20).unwrap(),
    ];
    FirstTableDefinition::capture_native("More", &columns, &mut budget(), &|| false).unwrap()
}

fn literal(before: &[u8], retained: bool) -> Vec<u8> {
    let mut nested = template(1);
    nested[..8].copy_from_slice(b"TCNT0007");
    nested[12..14].copy_from_slice(&7_u16.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    let mut bytes = b"TCAT0007".to_vec();
    bytes.extend(((64 + nested.len()) as u32).to_le_bytes());
    bytes.extend(7_u16.to_le_bytes());
    bytes.extend(u16::from(retained).to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend(hash.finalize());
    bytes.extend((nested.len() as u32).to_le_bytes());
    bytes.extend([0; 4]);
    bytes.extend(nested);
    bytes
}

#[test]
fn native_add_table_keeps_cohort_distinct_and_both_older_decoders_closed() {
    // Data framing deliberately does not claim that this inert image owns roles.
    let before = vec![0x5a; 17 * 4096];
    for (profile, retained) in [
        (AddTableSourceProfile::Mixed, false),
        (AddTableSourceProfile::Retained, true),
    ] {
        let expected = literal(&before, retained);
        let request = NativeAddTableIntent::new_native(
            definition(),
            profile,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(request.encode(&mut budget(), &|| false).unwrap(), expected);
        let decoded =
            NativeAddTableIntent::decode_native(&expected, &mut budget(), &|| false).unwrap();
        assert!(decoded.is_native());
        assert_eq!(decoded.source_profile(), profile);
        assert_eq!(decoded.table(), "More");
        assert_eq!(decoded.column_count(), 2);
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert!(matches!(
            NativeAddTableIntent::decode(&expected, &mut budget(), &|| false),
            Err(AddTableIntentError::UnsupportedVersion)
        ));
        assert!(matches!(
            NativeAddTableIntent::decode_retained(&expected, &mut budget(), &|| false),
            Err(AddTableIntentError::UnsupportedVersion)
        ));
    }
    assert!(matches!(
        NativeAddTableIntent::new_retained(definition(), &before, &mut budget(), &|| false),
        Err(AddTableIntentError::UnsupportedProfile)
    ));
    assert!(matches!(
        NativeAddTableIntent::new_mixed(definition(), &before, &mut budget(), &|| false),
        Err(AddTableIntentError::UnsupportedProfile)
    ));
}

#[test]
fn native_nested_old_family_and_unknown_cohort_fail_before_allocations() {
    let before = vec![0; 10 * 4096];
    let original = literal(&before, false);
    for (offset, value, category) in [(14, 2, 0), (64 + 7, b'6', 1), (64 + 12, 6, 2), (60, 1, 0)] {
        let mut malformed = original.clone();
        malformed[offset] = value;
        let result = NativeAddTableIntent::decode_native(
            &malformed,
            &mut ResourceBudget::new(0, 1000, 100_000),
            &|| false,
        );
        match category {
            0 => assert!(matches!(result, Err(AddTableIntentError::InvalidRecord))),
            1 => assert!(matches!(
                result,
                Err(AddTableIntentError::UnsupportedVersion)
            )),
            2 => assert!(matches!(
                result,
                Err(AddTableIntentError::UnsupportedProfile)
            )),
            _ => unreachable!(),
        }
    }
    let mut legacy = original;
    legacy[..8].copy_from_slice(b"TCAT0006");
    legacy[12..14].copy_from_slice(&6_u16.to_le_bytes());
    assert!(matches!(
        NativeAddTableIntent::decode_retained(&legacy, &mut budget(), &|| false),
        Err(AddTableIntentError::UnsupportedVersion)
    ));
}

#[test]
fn native_binding_covers_data_beyond_legacy_image_envelope() {
    let mut before = vec![0; 64 * 1024 * 1024 + 4096];
    let allowance = || ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 512 * 1024 * 1024);
    let request = NativeAddTableIntent::new_native(
        definition(),
        AddTableSourceProfile::Mixed,
        &before,
        &mut allowance(),
        &|| false,
    )
    .unwrap();
    request
        .verify_before(&before, &mut allowance(), &|| false)
        .unwrap();
    *before.last_mut().unwrap() = 1;
    assert!(matches!(
        request.verify_before(&before, &mut allowance(), &|| false),
        Err(AddTableIntentError::BeforeMismatch)
    ));
    assert!(
        request
            .verify_before(&before, &mut allowance(), &|| true)
            .is_err()
    );
    assert!(
        request
            .verify_before(&before, &mut ResourceBudget::new(u64::MAX, 0, 0), &|| false)
            .is_err()
    );
}

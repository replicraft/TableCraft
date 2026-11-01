//! Independent TCNI0004 framing; constructing data never grants native authority.
use super::{NativeFixedEditIntent, NativeFixedIntentGroup};
use crate::{
    FixedScalarUpdate, InspectError, NativeFittedEditIntent, NativeIntentError, NativeUpdateIntent,
    NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

fn original() -> NativeFixedEditIntent {
    let specifications = [
        (
            "Flag",
            vec![FixedScalarUpdate::new(
                1,
                Value::Boolean(false),
                Value::Boolean(true),
            )],
        ),
        (
            "Money",
            vec![FixedScalarUpdate::new(
                0,
                Value::Currency(Currency::from_scaled(i64::MIN)),
                Value::Currency(Currency::from_scaled(i64::MAX)),
            )],
        ),
        (
            "Date",
            vec![FixedScalarUpdate::new(
                0,
                Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
            )],
        ),
        (
            "I32",
            vec![FixedScalarUpdate::new(1, Value::Null, Value::Int32(-1))],
        ),
        ("Empty", vec![]),
    ];
    let groups = specifications
        .into_iter()
        .map(|(name, updates)| {
            NativeFixedIntentGroup::new(name, updates, &mut budget(), &|| false).unwrap()
        })
        .collect();
    NativeFixedEditIntent::new("Items", groups, b"original", &mut budget(), &|| false).unwrap()
}

fn literal() -> Vec<u8> {
    // Independent 72-byte header, five groups and four 26-byte assignments.
    type Group = (&'static str, Vec<(u64, u8, u64, u8, u64)>);
    let groups: [Group; 5] = [
        ("Flag", vec![(1, 1, 0, 1, 1)]),
        (
            "Money",
            vec![(0, 4, 0x8000_0000_0000_0000, 4, 0x7fff_ffff_ffff_ffff)],
        ),
        ("Date", vec![(0, 5, 0, 5, 0x8000_0000_0000_0000)]),
        ("I32", vec![(1, 0, 0, 2, 0xffff_ffff)]),
        ("Empty", vec![]),
    ];
    let mut wire = Vec::from(*b"TCNI0004");
    wire.extend_from_slice(&262_u32.to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&8_u64.to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update(8_u64.to_le_bytes());
    hash.update(b"original");
    wire.extend_from_slice(&hash.finalize());
    wire.extend_from_slice(&5_u32.to_le_bytes());
    wire.extend_from_slice(&5_u32.to_le_bytes());
    wire.extend_from_slice(&4_u32.to_le_bytes());
    wire.extend_from_slice(&0_u32.to_le_bytes());
    wire.extend_from_slice(b"Items");
    for (name, updates) in groups {
        wire.extend_from_slice(&(name.len() as u32).to_le_bytes());
        wire.extend_from_slice(&(updates.len() as u32).to_le_bytes());
        wire.extend_from_slice(&0_u32.to_le_bytes());
        wire.extend_from_slice(name.as_bytes());
        for (row, expected_tag, expected, replacement_tag, replacement) in updates {
            wire.extend_from_slice(&row.to_le_bytes());
            wire.push(expected_tag);
            wire.extend_from_slice(&expected.to_le_bytes());
            wire.push(replacement_tag);
            wire.extend_from_slice(&replacement.to_le_bytes());
        }
    }
    assert_eq!(wire.len(), 262);
    wire
}

#[test]
fn ordinary_fixed_wire_is_distinct_and_keeps_exact_original_body() {
    let original = original();
    let pointer = original.groups()[1].updates().as_ptr();
    let mut positive_wire = literal();
    positive_wire[..8].copy_from_slice(b"TCNI0003");
    positive_wire[12..14].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        original.encode(&mut budget(), &|| false).unwrap(),
        positive_wire
    );
    let intent = NativeUpdateIntent::from_ordinary_fixed(original);
    assert_eq!(intent.profile(), NativeUpdateProfile::Ace16OrdinaryFixed);
    assert!(intent.int32().is_none());
    let fixed = intent.fixed().unwrap();
    assert_eq!(fixed.groups()[1].updates().as_ptr(), pointer);
    assert_eq!(fixed.groups().len(), 5);
    assert!(fixed.groups()[4].updates().is_empty());
    assert_ne!(
        fixed.groups()[2].updates()[0].expected(),
        fixed.groups()[2].updates()[0].value()
    );
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal());
    let decoded = NativeUpdateIntent::decode(&literal(), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.profile(), NativeUpdateProfile::Ace16OrdinaryFixed);
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal());
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"originaL", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
    assert_eq!(
        NativeUpdateIntent::MAX_ENCODED_BYTES,
        NativeFittedEditIntent::MAX_ENCODED_BYTES
    );
}

#[test]
fn ordinary_and_positive_fixed_profiles_never_cross_decode() {
    let ordinary = literal();
    assert!(matches!(
        NativeFixedEditIntent::decode(&ordinary, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    let positive = original().encode(&mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeFixedEditIntent::decode_ordinary(&positive, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
    for (mut wire, profile) in [(ordinary, 3_u16), (positive, 4_u16)] {
        wire[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false),
            Err(NativeIntentError::UnsupportedProfile)
        ));
    }
}

#[test]
fn ordinary_fixed_codec_rejects_malformed_carriers_and_bounded_work() {
    let wire = literal();
    // Flags/reserved framing, Boolean upper padding and inactive NULL payload.
    for at in [14, 68, 103, 228] {
        let mut changed = wire.clone();
        changed[at] = 1;
        assert!(NativeUpdateIntent::decode(&changed, &mut budget(), &|| false).is_err());
    }
    let mut invalid_date = wire.clone();
    invalid_date[196..204].fill(0xff);
    assert!(NativeUpdateIntent::decode(&invalid_date, &mut budget(), &|| false).is_err());
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(NativeUpdateIntent::decode(&trailing, &mut budget(), &|| false).is_err());
    for mut limited in [
        ResourceBudget::new(0, 100_000, 16_000_000),
        ResourceBudget::new(16_000_000, 100_000, 0),
    ] {
        assert!(matches!(
            NativeUpdateIntent::decode(&wire, &mut limited, &|| false),
            Err(NativeIntentError::Inspect(InspectError::Limit(_)))
        ));
    }
    // Intent decoding validates request metadata, not database rows. Native
    // content qualification and replay own the later row allowance.
    let mut metadata_only = ResourceBudget::new(16_000_000, 0, 16_000_000);
    let decoded = NativeUpdateIntent::decode(&wire, &mut metadata_only, &|| false).unwrap();
    assert_eq!(decoded.encode(&mut metadata_only, &|| false).unwrap(), wire);
    assert_eq!(metadata_only.remaining(Resource::Rows), 0);

    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| true),
        Err(NativeIntentError::Inspect(InspectError::Cancelled))
    ));
}

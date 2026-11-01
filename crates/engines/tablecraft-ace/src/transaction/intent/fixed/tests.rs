use super::{NativeFixedEditIntent, NativeFixedIntentGroup};
use crate::{
    FixedScalarUpdate, InspectError, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

fn groups() -> Vec<NativeFixedIntentGroup> {
    let specifications = [
        (
            "Flag",
            vec![FixedScalarUpdate::new(
                0,
                Value::Boolean(false),
                Value::Boolean(true),
            )],
        ),
        (
            "I32",
            vec![FixedScalarUpdate::new(
                2,
                Value::Int32(-1),
                Value::Int32(i32::MIN),
            )],
        ),
        (
            "I64",
            vec![
                FixedScalarUpdate::new(1, Value::Int64(i64::MAX), Value::Null),
                FixedScalarUpdate::new(0, Value::Null, Value::Int64(i64::MIN)),
            ],
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
            vec![
                FixedScalarUpdate::new(
                    0,
                    Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                    Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
                ),
                FixedScalarUpdate::new(
                    1,
                    Value::DateTime(LegacyDateTime::from_bits(0x3fd0_0000_0000_0000).unwrap()),
                    Value::DateTime(LegacyDateTime::from_bits(0xbfd0_0000_0000_0000).unwrap()),
                ),
            ],
        ),
        ("Empty", vec![]),
    ];
    specifications
        .into_iter()
        .map(|(name, updates)| {
            NativeFixedIntentGroup::new(name, updates, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

fn literal() -> Vec<u8> {
    // Independent framing: 72-byte header, UTF-8 names, 12-byte group headers,
    // original row u64 and two [tag, exact LE8] carriers per 26-byte request.
    type LiteralGroup = (&'static str, Vec<(u64, u8, u64, u8, u64)>);
    let specifications: [LiteralGroup; 6] = [
        ("Flag", vec![(0, 1, 0, 1, 1)]),
        ("I32", vec![(2, 2, 0xffff_ffff, 2, 0x8000_0000)]),
        (
            "I64",
            vec![
                (1, 3, 0x7fff_ffff_ffff_ffff, 0, 0),
                (0, 0, 0, 3, 0x8000_0000_0000_0000),
            ],
        ),
        (
            "Money",
            vec![(0, 4, 0x8000_0000_0000_0000, 4, 0x7fff_ffff_ffff_ffff)],
        ),
        (
            "Date",
            vec![
                (0, 5, 0, 5, 0x8000_0000_0000_0000),
                (1, 5, 0x3fd0_0000_0000_0000, 5, 0xbfd0_0000_0000_0000),
            ],
        ),
        ("Empty", vec![]),
    ];
    let mut raw = Vec::from(*b"TCNI0003");
    raw.extend_from_slice(&355_u32.to_le_bytes());
    raw.extend_from_slice(&[3, 0, 0, 0]);
    raw.extend_from_slice(&8_u64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update(8_u64.to_le_bytes());
    digest.update(b"original");
    raw.extend_from_slice(&digest.finalize());
    raw.extend_from_slice(&5_u32.to_le_bytes());
    raw.extend_from_slice(&6_u32.to_le_bytes());
    raw.extend_from_slice(&7_u32.to_le_bytes());
    raw.extend_from_slice(&0_u32.to_le_bytes());
    raw.extend_from_slice(b"Items");
    for (name, updates) in specifications {
        raw.extend_from_slice(&(name.len() as u32).to_le_bytes());
        raw.extend_from_slice(&(updates.len() as u32).to_le_bytes());
        raw.extend_from_slice(&0_u32.to_le_bytes());
        raw.extend_from_slice(name.as_bytes());
        for (row, before_tag, before, after_tag, after) in updates {
            raw.extend_from_slice(&row.to_le_bytes());
            raw.push(before_tag);
            raw.extend_from_slice(&before.to_le_bytes());
            raw.push(after_tag);
            raw.extend_from_slice(&after.to_le_bytes());
        }
    }
    assert_eq!(raw.len(), 355);
    raw
}

#[test]
fn original_fixed_literal_keeps_exact_carriers_empty_groups_and_typed_body() {
    let expected = literal();
    let owned = groups();
    let original_pointer = owned[2].updates().as_ptr();
    let original =
        NativeFixedEditIntent::new("Items", owned, b"original", &mut budget(), &|| false).unwrap();
    assert_eq!(original.groups()[2].updates().as_ptr(), original_pointer);
    assert_eq!(original.encode(&mut budget(), &|| false).unwrap(), expected);
    let decoded = NativeUpdateIntent::decode(&expected, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16BeforeChangeFixed
    );
    assert!(decoded.int32().is_none());
    let fixed = decoded.fixed().unwrap();
    assert_eq!(fixed.table(), "Items");
    assert_eq!(fixed.groups().len(), 6);
    assert!(fixed.groups()[5].updates().is_empty());
    assert_eq!(fixed.groups()[2].updates()[0].value(), &Value::Null);
    assert_eq!(fixed.groups()[2].updates()[1].expected(), &Value::Null);
    assert_ne!(
        fixed.groups()[4].updates()[0].expected(),
        fixed.groups()[4].updates()[0].value()
    );
    assert_ne!(
        fixed.groups()[4].updates()[1].expected(),
        fixed.groups()[4].updates()[1].value()
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), expected);
    decoded
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        decoded.verify_before(b"originaL", &mut budget(), &|| false),
        Err(NativeIntentError::BeforeMismatch)
    ));
}

#[test]
fn malformed_fixed_records_never_alias_another_original_request() {
    let raw = literal();
    // Header flags/reserved, count, first group reserved and canonical carrier padding.
    for (at, value) in [(14, 1), (68, 1), (64, 8), (85, 1), (101, 255), (103, 1)] {
        let mut changed = raw.clone();
        changed[at] = value;
        assert!(
            NativeUpdateIntent::decode(&changed, &mut budget(), &|| false).is_err(),
            "offset {at}"
        );
    }
    let mut invalid_date = raw.clone();
    invalid_date[295..303].fill(0xff);
    assert!(NativeUpdateIntent::decode(&invalid_date, &mut budget(), &|| false).is_err());
    let mut inactive_null = raw.clone();
    inactive_null[193] = 1;
    assert!(NativeUpdateIntent::decode(&inactive_null, &mut budget(), &|| false).is_err());
    for end in [0, 7, 71, 94, 354] {
        assert!(NativeUpdateIntent::decode(&raw[..end], &mut budget(), &|| false).is_err());
    }
    let mut trailing = raw.clone();
    trailing.push(0);
    assert!(NativeUpdateIntent::decode(&trailing, &mut budget(), &|| false).is_err());
    let mut wrong_profile = raw.clone();
    wrong_profile[12] = 2;
    assert!(matches!(
        NativeUpdateIntent::decode(&wrong_profile, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedProfile)
    ));
    let mut unknown = raw;
    unknown[..8].copy_from_slice(b"TCNI0004");
    assert!(matches!(
        NativeUpdateIntent::decode(&unknown, &mut budget(), &|| false),
        Err(NativeIntentError::UnsupportedVersion)
    ));
}

#[test]
fn fixed_intent_metadata_work_is_bounded_and_cancellable_without_visiting_rows() {
    let raw = literal();
    for mut limited in [
        ResourceBudget::new(0, 100_000, 16_000_000),
        ResourceBudget::new(16_000_000, 100_000, 0),
    ] {
        assert!(matches!(
            NativeUpdateIntent::decode(&raw, &mut limited, &|| false),
            Err(NativeIntentError::Inspect(InspectError::Limit(_)))
        ));
    }
    // Intent decoding visits metadata and assignments, not database rows.
    // Actual row work is charged by native planning and recovery qualification.
    let mut metadata_only = ResourceBudget::new(16_000_000, 0, 16_000_000);
    NativeUpdateIntent::decode(&raw, &mut metadata_only, &|| false).unwrap();
    assert_eq!(metadata_only.remaining(Resource::Rows), 0);
    assert!(matches!(
        NativeUpdateIntent::decode(&raw, &mut budget(), &|| true),
        Err(NativeIntentError::Inspect(InspectError::Cancelled))
    ));
    let too_many = u32::MAX.to_le_bytes();
    let mut excessive = raw;
    excessive[60..64].copy_from_slice(&too_many);
    assert!(NativeUpdateIntent::decode(&excessive, &mut budget(), &|| false).is_err());
    assert!(
        NativeFixedIntentGroup::new(
            "Bad",
            vec![FixedScalarUpdate::new(
                0,
                Value::Text(String::new()),
                Value::Null
            )],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[path = "tests/uniqueness.rs"]
mod uniqueness;

//! Independent TCNI17 framing and explicit retained request-data bounds.
use crate::{
    FixedScalarUpdate, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates,
    NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn groups(expected: Option<&str>, replacement: Option<&str>) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            "NextText",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                expected.map_or(Value::Null, |text| Value::Text(text.into())),
                replacement.map_or(Value::Null, |text| Value::Text(text.into())),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal(before: &[u8], expected: Option<&str>, replacement: Option<&str>) -> Vec<u8> {
    let payload = expected.map_or(0, str::len) + replacement.map_or(0, str::len);
    let mut wire = b"TCNI0017".to_vec();
    wire.extend((134_u32 + payload as u32).to_le_bytes());
    wire.extend([17, 0, 0, 0]);
    wire.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    wire.extend(digest.finalize());
    for number in [6_u32, 1, 1, 0] {
        wire.extend(number.to_le_bytes());
    }
    wire.extend(b"Table1");
    wire.extend((56_u32 + payload as u32).to_le_bytes());
    wire.extend(8_u32.to_le_bytes());
    wire.extend(1_u32.to_le_bytes());
    wire.extend([1, 0, 0, 0]);
    wire.extend([0; 4]);
    wire.extend(b"NextText");
    wire.extend((28_u32 + payload as u32).to_le_bytes());
    wire.extend([0; 8]);
    for cell in [expected, replacement] {
        wire.extend([if cell.is_some() { 6 } else { 0 }, 0, 0, 0]);
        wire.extend((cell.map_or(0, str::len) as u32).to_le_bytes());
        if let Some(text) = cell {
            wire.extend(text.as_bytes());
        }
    }
    assert_eq!(wire.len(), 134 + payload);
    wire
}

#[test]
fn retained_update_data_matches_literal_wire_and_preserves_explicit_family_identity() {
    // Arbitrary aligned bytes bind inert requests; they do not qualify a database.
    let before = [0x6b; 4096];
    for (expected, replacement) in [
        (None, Some("日本語🦀")),
        (Some("old"), None),
        (Some(""), Some("")),
    ] {
        let wire = literal(&before, expected, replacement);
        let intent = NativeFittedEditIntent::new_retained(
            "Table1",
            groups(expected, replacement),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            intent.encode_retained(&mut budget(), &|| false).unwrap(),
            wire
        );
        assert!(intent.encode(&mut budget(), &|| false).is_err());
        let decoded =
            NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
        decoded
            .verify_before(&before, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            decoded.encode_retained(&mut budget(), &|| false).unwrap(),
            wire
        );
        assert!(NativeFittedEditIntent::decode(&wire, &mut budget(), &|| false).is_err());
        let dispatched = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        assert_eq!(dispatched.profile(), NativeUpdateProfile::Ace16RetainedRows);
        assert_eq!(dispatched.encode(&mut budget(), &|| false).unwrap(), wire);
        for version in [5_u8, 16] {
            let mut older = wire.to_vec();
            older[6..8].copy_from_slice(if version == 5 { b"05" } else { b"16" });
            older[12] = version;
            assert!(
                NativeFittedEditIntent::decode_retained(&older, &mut budget(), &|| false).is_err()
            );
        }
    }
}

#[test]
fn retained_update_preflight_rejects_malformed_frames_before_allocating_owned_values() {
    let wire = literal(&[0; 4096], None, Some("日本語🦀"));
    for end in 0..wire.len() {
        let mut work = budget();
        let before = work.remaining(Resource::Bytes);
        assert!(
            NativeFittedEditIntent::decode_retained(&wire[..end], &mut work, &|| false).is_err()
        );
        assert_eq!(work.remaining(Resource::Bytes), before);
    }
    for (offset, value) in [(8, 0), (12, 16), (14, 1), (68, 1), (90, 2), (94, 1)] {
        let mut invalid = wire.to_vec();
        invalid[offset] = value;
        let mut work = budget();
        let before = work.remaining(Resource::Bytes);
        assert!(
            NativeFittedEditIntent::decode_retained(&invalid, &mut work, &|| false).is_err(),
            "offset {offset}"
        );
        assert_eq!(work.remaining(Resource::Bytes), before);
    }
    for mut work in [
        ResourceBudget::new(0, 0, 500_000_000),
        ResourceBudget::new(512 * 1024 * 1024, 0, 0),
    ] {
        assert!(NativeFittedEditIntent::decode_retained(&wire, &mut work, &|| false).is_err());
    }
    assert!(NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| true).is_err());
}

#[test]
fn retained_update_data_uses_wire_widths_instead_of_legacy_image_and_carrier_quotas() {
    let before = vec![0; 64 * 1024 * 1024 + 4096];
    let text = "x".repeat(8193);
    let intent = NativeFittedEditIntent::new_retained(
        "Table1",
        groups(None, Some(&text)),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wire = intent.encode_retained(&mut budget(), &|| false).unwrap();
    assert_eq!(wire, literal(&before, None, Some(&text)));
    let decoded = NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert!(
        NativeFittedEditIntent::new(
            "Table1",
            groups(None, Some(&text)),
            &before,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for version in [5_u8, 16] {
        let mut older = wire.to_vec();
        older[6..8].copy_from_slice(if version == 5 { b"05" } else { b"16" });
        older[12] = version;
        assert!(NativeUpdateIntent::decode(&older, &mut budget(), &|| false).is_err());
    }
    assert!(
        NativeFittedEditIntent::new_retained(
            "Table1",
            groups(None, None),
            &before[..4095],
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn retained_update_request_count_and_sparse_ordinals_have_no_legacy_batch_quota() {
    // This exercises original data only, independently of native target capacity.
    let cells = (0..16_385)
        .map(|row| FixedScalarUpdate::new(row * 1000, Value::Int32(1), Value::Int32(2)))
        .collect();
    let group = NativeFittedIntentGroup::new_retained(
        "Score",
        NativeFittedUpdates::Fixed(cells),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let intent = NativeFittedEditIntent::new_retained(
        "Table1",
        vec![group],
        &[0; 4096],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let wire = intent.encode_retained(&mut budget(), &|| false).unwrap();
    let decoded = NativeFittedEditIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.groups()[0].updates().len(), 16_385);
    assert_eq!(
        decoded.encode_retained(&mut budget(), &|| false).unwrap(),
        wire
    );
}

#[cfg(feature = "ace16-create")]
mod recovery;

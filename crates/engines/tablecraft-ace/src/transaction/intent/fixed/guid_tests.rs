use super::{NativeFixedEditIntent, NativeFixedIntentGroup, carrier, codec};
use crate::{
    FixedScalarUpdate, InspectError, NativeImage, NativeIntentError, NativeUpdateIntent,
    NativeUpdateProfile,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Single, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

#[test]
fn guid_original_cells_are_canonical_and_old_cells_have_canonical_wide_padding() {
    let canonical = [
        0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
    ];
    let value = Value::Guid(Guid::from_bytes(canonical));
    let mut literal = [0_u8; 17];
    literal[0] = 10;
    literal[1..].copy_from_slice(&canonical);
    assert_eq!(carrier::encode_guid(&value).unwrap(), literal);
    assert_eq!(carrier::decode_guid(&literal).unwrap(), value);
    assert_eq!(
        carrier::encode(&value),
        Err(NativeIntentError::InvalidRecord)
    );
    assert_eq!(
        carrier::encode_ieee(&value),
        Err(NativeIntentError::InvalidRecord)
    );
    assert_eq!(
        carrier::encode_scalar(&value),
        Err(NativeIntentError::InvalidRecord)
    );

    for value in [
        Value::Null,
        Value::Byte(255),
        Value::Int16(-32768),
        Value::Single(Single::from_bits(0x7f81_2345)),
    ] {
        let old = carrier::encode_scalar(&value).unwrap();
        let mut expected = [0_u8; 17];
        expected[..9].copy_from_slice(&old);
        assert_eq!(carrier::encode_guid(&value).unwrap(), expected);
        assert_eq!(carrier::decode_guid(&expected).unwrap(), value);
        for offset in 9..17 {
            let mut invalid = expected;
            invalid[offset] = 1;
            assert_eq!(
                carrier::decode_guid(&invalid),
                Err(NativeIntentError::InvalidRecord)
            );
        }
    }
}

#[test]
fn guid_original_record_roundtrips_without_expanding_closed_old_dialects() {
    let update =
        || FixedScalarUpdate::new(2, Value::Null, Value::Guid(Guid::from_bytes([0x5a; 16])));
    let group = NativeFixedIntentGroup::new_ordinary_guid(
        "GuidValue",
        vec![update()],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let original =
        NativeFixedEditIntent::new("Items", vec![group], b"original", &mut budget(), &|| false)
            .unwrap();
    assert_eq!(
        original.encode(&mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    );
    assert_eq!(
        original.encode_ordinary(&mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    );
    assert_eq!(
        original.encode_ieee(&mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    );
    assert_eq!(
        original.encode_scalar(&mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord)
    );
    let intent = NativeUpdateIntent::from_ordinary_guid_fixed(original);
    let wire = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&wire[..8], b"TCNI0010");
    assert_eq!(&wire[12..14], &10_u16.to_le_bytes());
    assert_eq!(wire.len(), 72 + 5 + 12 + 9 + 42);
    let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16OrdinaryGuidFixed
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), wire);
    decoded
        .fixed()
        .unwrap()
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();
    for (magic, profile) in [
        (b"TCNI0003", 3_u16),
        (b"TCNI0004", 4),
        (b"TCNI0008", 8),
        (b"TCNI0009", 9),
    ] {
        let mut invalid = wire.clone();
        invalid[..8].copy_from_slice(magic);
        invalid[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false).is_err());
    }
}

#[test]
fn wider_original_cells_do_not_raise_legacy_fast_record_length_bounds() {
    assert_eq!(
        codec::MAX_GUID_RECORD_BYTES - codec::MAX_RECORD_BYTES,
        NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS * 16
    );
    for (magic, profile) in [
        (b"TCNI0003", 3_u16),
        (b"TCNI0004", 4),
        (b"TCNI0008", 8),
        (b"TCNI0009", 9),
    ] {
        let mut oversized = vec![0; codec::MAX_RECORD_BYTES + 1];
        oversized[..8].copy_from_slice(magic);
        oversized[12..14].copy_from_slice(&profile.to_le_bytes());
        let mut work = ResourceBudget::new(0, 0, 1);
        assert!(matches!(
            NativeUpdateIntent::decode(&oversized, &mut work, &|| false),
            Err(NativeIntentError::InvalidRecord)
        ));
        assert_eq!(work.remaining(Resource::Steps), 0);
        assert_eq!(work.remaining(Resource::Bytes), 0);
    }
    let mut guid_width = vec![0; codec::MAX_RECORD_BYTES + 1];
    guid_width[..8].copy_from_slice(b"TCNI0010");
    guid_width[12..14].copy_from_slice(&10_u16.to_le_bytes());
    let mut work = ResourceBudget::new(0, 0, 1);
    assert!(matches!(
        NativeUpdateIntent::decode(&guid_width, &mut work, &|| false),
        Err(NativeIntentError::Inspect(InspectError::Limit(_)))
    ));
}

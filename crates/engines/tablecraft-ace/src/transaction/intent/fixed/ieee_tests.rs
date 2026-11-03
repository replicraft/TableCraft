use super::{NativeFixedEditIntent, NativeFixedIntentGroup, carrier};
use crate::{FixedScalarUpdate, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, LegacyDateTime, Single, Value},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 1_000_000)
}

#[test]
fn ordinary_ieee_cells_preserve_bits_and_legacy_tags_stay_closed() {
    for bits in [0_u32, 0x8000_0000, 1, 0x7f81_2345, 0xffc1_2345] {
        let value = Value::Single(Single::from_bits(bits));
        let mut literal = [0_u8; 9];
        literal[0] = 6;
        literal[1..5].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(carrier::encode_ieee(&value).unwrap(), literal);
        assert_eq!(carrier::decode_ieee(&literal).unwrap(), value);
        assert_eq!(
            carrier::encode(&value),
            Err(NativeIntentError::InvalidRecord)
        );
        assert_eq!(
            carrier::decode(&literal),
            Err(NativeIntentError::InvalidRecord)
        );

        for offset in 5..9 {
            let mut invalid = literal;
            invalid[offset] = 1;
            assert_eq!(
                carrier::decode_ieee(&invalid),
                Err(NativeIntentError::InvalidRecord)
            );
        }
    }

    for bits in [
        0_u64,
        0x8000_0000_0000_0000,
        1,
        0x000f_ffff_ffff_ffff,
        0xfff8_1234_5678_9abc,
    ] {
        let value = Value::Double(Double::from_bits(bits));
        let mut literal = [0_u8; 9];
        literal[0] = 7;
        literal[1..].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(carrier::encode_ieee(&value).unwrap(), literal);
        assert_eq!(carrier::decode_ieee(&literal).unwrap(), value);
        assert_eq!(
            carrier::encode(&value),
            Err(NativeIntentError::InvalidRecord)
        );
        assert_eq!(
            carrier::decode(&literal),
            Err(NativeIntentError::InvalidRecord)
        );
    }

    let mut unknown = [0_u8; 9];
    unknown[0] = 8;
    assert_eq!(
        carrier::decode_ieee(&unknown),
        Err(NativeIntentError::InvalidRecord)
    );
}

#[test]
fn extended_cells_leave_every_legacy_representation_and_padding_unchanged() {
    for value in [
        Value::Null,
        Value::Boolean(false),
        Value::Boolean(true),
        Value::Int32(i32::MIN),
        Value::Int64(i64::MIN),
        Value::Currency(Currency::from_scaled(i64::MIN)),
        Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
    ] {
        let legacy = carrier::encode(&value).unwrap();
        assert_eq!(carrier::encode_ieee(&value).unwrap(), legacy);
        assert_eq!(carrier::decode_ieee(&legacy).unwrap(), value);
    }

    for tag in [0, 1, 2] {
        let mut invalid = [0_u8; 9];
        invalid[0] = tag;
        invalid[8] = 1;
        assert_eq!(
            carrier::decode_ieee(&invalid),
            Err(NativeIntentError::InvalidRecord)
        );
    }
}

#[test]
fn extended_original_record_roundtrips_without_reinterpreting_legacy_profiles() {
    let update = || {
        FixedScalarUpdate::new(
            2,
            Value::Single(Single::from_bits(1)),
            Value::Single(Single::from_bits(0x7f81_2345)),
        )
    };
    assert!(matches!(
        NativeFixedIntentGroup::new("SingleValue", vec![update()], &mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord),
    ));

    let group = NativeFixedIntentGroup::new_ordinary_ieee(
        "SingleValue",
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

    let intent = NativeUpdateIntent::from_ordinary_ieee_fixed(original);
    let encoded = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&encoded[..8], b"TCNI0008");
    assert_eq!(&encoded[12..14], &8_u16.to_le_bytes());
    let decoded = NativeUpdateIntent::decode(&encoded, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16OrdinaryIeeeFixed
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), encoded);
    assert!(
        decoded
            .fixed()
            .unwrap()
            .verify_before(b"original", &mut budget(), &|| false)
            .is_ok()
    );

    for (magic, profile) in [(b"TCNI0003", 3_u16), (b"TCNI0004", 4)] {
        let mut invalid = encoded.clone();
        invalid[..8].copy_from_slice(magic);
        invalid[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false),
            Err(NativeIntentError::InvalidRecord),
        ));
    }
}

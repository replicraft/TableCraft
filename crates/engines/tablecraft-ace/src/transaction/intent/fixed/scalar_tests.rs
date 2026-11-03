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
fn ordinary_scalar_cells_use_narrow_native_payloads_and_canonical_zero_padding() {
    for (value, tag, payload) in [
        (Value::Byte(0), 8, vec![0]),
        (Value::Byte(255), 8, vec![255]),
        (Value::Int16(-32768), 9, vec![0, 128]),
        (Value::Int16(-1), 9, vec![255, 255]),
        (Value::Int16(0), 9, vec![0, 0]),
        (Value::Int16(32767), 9, vec![255, 127]),
    ] {
        let mut literal = [0_u8; 9];
        literal[0] = tag;
        literal[1..1 + payload.len()].copy_from_slice(&payload);
        assert_eq!(carrier::encode_scalar(&value).unwrap(), literal);
        assert_eq!(carrier::decode_scalar(&literal).unwrap(), value);
        assert_eq!(
            carrier::encode(&value),
            Err(NativeIntentError::InvalidRecord)
        );
        assert_eq!(
            carrier::encode_ieee(&value),
            Err(NativeIntentError::InvalidRecord)
        );
        assert_eq!(
            carrier::decode(&literal),
            Err(NativeIntentError::InvalidRecord)
        );
        assert_eq!(
            carrier::decode_ieee(&literal),
            Err(NativeIntentError::InvalidRecord)
        );

        for offset in 1 + payload.len()..9 {
            let mut invalid = literal;
            invalid[offset] = 255;
            assert_eq!(
                carrier::decode_scalar(&invalid),
                Err(NativeIntentError::InvalidRecord)
            );
        }
    }
    let mut unknown = [0_u8; 9];
    unknown[0] = 10;
    assert_eq!(
        carrier::decode_scalar(&unknown),
        Err(NativeIntentError::InvalidRecord)
    );
}

#[test]
fn cumulative_scalar_cells_preserve_every_legacy_and_ieee_representation() {
    for value in [
        Value::Null,
        Value::Boolean(false),
        Value::Boolean(true),
        Value::Int32(i32::MIN),
        Value::Int64(i64::MIN),
        Value::Currency(Currency::from_scaled(i64::MIN)),
        Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
        Value::Single(Single::from_bits(0x8000_0000)),
        Value::Single(Single::from_bits(0x7f81_2345)),
        Value::Double(Double::from_bits(0xfff8_1234_5678_9abc)),
    ] {
        let old = carrier::encode_ieee(&value).unwrap();
        assert_eq!(carrier::encode_scalar(&value).unwrap(), old);
        assert_eq!(carrier::decode_scalar(&old).unwrap(), value);
    }
}

#[test]
fn cumulative_original_record_does_not_expand_old_fixed_or_ieee_dialects() {
    let update = || FixedScalarUpdate::new(2, Value::Int16(-32768), Value::Int16(32767));
    assert!(matches!(
        NativeFixedIntentGroup::new("Int16Value", vec![update()], &mut budget(), &|| false),
        Err(NativeIntentError::InvalidRecord),
    ));
    assert!(matches!(
        NativeFixedIntentGroup::new_ordinary_ieee(
            "Int16Value",
            vec![update()],
            &mut budget(),
            &|| false
        ),
        Err(NativeIntentError::InvalidRecord),
    ));
    let group = NativeFixedIntentGroup::new_ordinary_scalar(
        "Int16Value",
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

    let intent = NativeUpdateIntent::from_ordinary_scalar_fixed(original);
    let encoded = intent.encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&encoded[..8], b"TCNI0009");
    assert_eq!(&encoded[12..14], &9_u16.to_le_bytes());
    let decoded = NativeUpdateIntent::decode(&encoded, &mut budget(), &|| false).unwrap();
    assert_eq!(
        decoded.profile(),
        NativeUpdateProfile::Ace16OrdinaryScalarFixed
    );
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), encoded);
    decoded
        .fixed()
        .unwrap()
        .verify_before(b"original", &mut budget(), &|| false)
        .unwrap();

    for (magic, profile) in [(b"TCNI0003", 3_u16), (b"TCNI0004", 4), (b"TCNI0008", 8)] {
        let mut invalid = encoded.clone();
        invalid[..8].copy_from_slice(magic);
        invalid[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            NativeUpdateIntent::decode(&invalid, &mut budget(), &|| false),
            Err(NativeIntentError::InvalidRecord)
        ));
    }
}

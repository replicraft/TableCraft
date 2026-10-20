#![cfg(feature = "serde")]

use proptest::prelude::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value,
};

fn round_trip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap()
}

#[test]
fn every_scalar_variant_round_trips_at_its_boundaries() {
    let values = [
        Value::Null,
        Value::Boolean(true),
        Value::Byte(u8::MAX),
        Value::Int16(i16::MIN),
        Value::Int32(i32::MAX),
        Value::Int64(i64::MIN),
        Value::Int64(i64::MAX),
        Value::Int64(9_007_199_254_740_993),
        Value::Single(Single::from_bits(0x7fc00001)),
        Value::Single(Single::from_float(-0.0)),
        Value::Double(Double::from_bits(0x7ff8000000000001)),
        Value::Double(Double::from_float(f64::INFINITY)),
        Value::Currency(Currency::from_scaled(i64::MIN)),
        Value::Decimal(Decimal::new(Decimal::MAX_COEFFICIENT, 28).unwrap()),
        Value::Decimal(Decimal::new(-1200, 3).unwrap()),
        Value::Text("\0é漢字🙂".into()),
        Value::Text(String::new()),
        Value::Binary(vec![0, 255, 128]),
        Value::Guid(Guid::from_bytes([255; 16])),
        Value::DateTime(LegacyDateTime::from_ole_days(-0.0).unwrap()),
        Value::DateTime(LegacyDateTime::from_ole_days(-2.5).unwrap()),
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
        Value::DateTimeExtended(
            ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
        ),
    ];
    for value in values {
        assert_eq!(round_trip(&value), value);
    }
}

#[test]
fn serde_representations_preserve_carrier_types_without_float_coercion() {
    assert_eq!(
        serde_json::to_value(Value::Int64(i64::MAX)).unwrap(),
        json!({"type":"int64","value":i64::MAX})
    );
    assert_eq!(
        serde_json::to_value(Value::Currency(Currency::from_scaled(i64::MIN))).unwrap(),
        json!({"type":"currency","value":i64::MIN})
    );
    assert_eq!(
        serde_json::to_value(Value::Decimal(Decimal::new(1200, 3).unwrap())).unwrap(),
        json!({"type":"decimal","value":{"coefficient":1200,"scale":3}})
    );
    assert_eq!(
        serde_json::to_value(Value::Double(Double::from_float(-0.0))).unwrap(),
        json!({"type":"double","value":(-0.0_f64).to_bits()})
    );
    assert_eq!(
        serde_json::to_value(Value::DateTime(
            LegacyDateTime::from_ole_days(-0.0).unwrap()
        ))
        .unwrap(),
        json!({"type":"date_time","value":(-0.0_f64).to_bits()})
    );
    assert_eq!(
        serde_json::to_value(Value::Null).unwrap(),
        json!({"type":"null"})
    );
    assert_ne!(
        serde_json::to_value(Value::Null).unwrap(),
        serde_json::to_value(Value::Text(String::new())).unwrap()
    );
}

#[test]
fn invalid_decimal_encodings_cannot_bypass_constructors() {
    for text in [
        r#"{"coefficient":79228162514264337593543950336,"scale":0}"#,
        r#"{"coefficient":-79228162514264337593543950336,"scale":0}"#,
        r#"{"coefficient":0,"scale":29}"#,
        r#"{"coefficient":0,"scale":-1}"#,
        r#"{"coefficient":"1","scale":0}"#,
        r#"{"coefficient":1,"scale":0,"unexpected":true}"#,
        r#"{"coefficient":1}"#,
    ] {
        assert!(serde_json::from_str::<Decimal>(text).is_err());
    }
    let text = r#"{"type":"decimal","value":{"coefficient":0,"scale":29}}"#;
    assert!(serde_json::from_str::<Value>(text).is_err());
}

#[test]
fn invalid_dates_and_bit_carriers_are_rejected() {
    for bits in [
        f64::NAN.to_bits(),
        f64::INFINITY.to_bits(),
        (-657435.0_f64).to_bits(),
        2958466.0_f64.to_bits(),
    ] {
        assert!(serde_json::from_value::<LegacyDateTime>(json!(bits)).is_err());
    }
    for text in [
        "0",
        "-000000000000001",
        "000000000000000g",
        "é00000000000000",
    ] {
        assert!(serde_json::from_value::<Double>(json!(text)).is_err());
    }
    assert!(serde_json::from_value::<Guid>(json!("0000")).is_err());
    assert!(serde_json::from_value::<Guid>(json!(vec![0; 16])).is_ok());
    let valid = json!({"year":2000,"month":2,"day":29,"hour":23,"minute":59,"second":59,"fraction_100ns":9_999_999});
    assert!(serde_json::from_value::<ExtendedDateTime>(valid.clone()).is_ok());
    for (field, bad) in [
        ("year", 1900),
        ("month", 13),
        ("day", 30),
        ("hour", 24),
        ("minute", 60),
        ("second", 60),
        ("fraction_100ns", 10_000_000),
    ] {
        let mut value = valid.clone();
        value[field] = json!(bad);
        assert!(serde_json::from_value::<ExtendedDateTime>(value).is_err());
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5441_424c_4543_5241),
        ..proptest::test_runner::Config::default()
    })]

    #[test]
    fn float_bits_round_trip_including_arbitrary_nan_payloads(single in any::<u32>(), double in any::<u64>()) {
        let value=Value::Single(Single::from_bits(single));
        prop_assert_eq!(round_trip(&value),value);
        let value=Value::Double(Double::from_bits(double));
        prop_assert_eq!(round_trip(&value),value);
    }

    #[test]
    fn decimal_representations_round_trip_without_normalizing_scale(coefficient in -Decimal::MAX_COEFFICIENT..=Decimal::MAX_COEFFICIENT,scale in 0_u8..=28) {
        let value=Value::Decimal(Decimal::new(coefficient,scale).unwrap());
        prop_assert_eq!(round_trip(&value),value);
    }

    #[test]
    fn integers_and_unicode_round_trip(integer in any::<i64>(), text in proptest::collection::vec(any::<char>(),0..64)) {
        let value=Value::Int64(integer);
        prop_assert_eq!(round_trip(&value),value);
        let value=Value::Text(text.into_iter().collect());
        prop_assert_eq!(round_trip(&value),value);
    }
}

//! CVar over already materialized values, without VBA variable/object state.
use tablecraft_core::values::Value;

/// Return the argument data value unchanged, retaining its carrier and ownership.
/// No text parsing, numeric promotion, allocation or object activation occurs.
/// This does not construct a VBA variable or extend SQL array/binary admission.
pub fn cvar(value: Value) -> Value {
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single};

    #[test]
    fn original_carriers_and_ieee_representations_are_preserved() {
        for value in [
            Value::Null,
            Value::Boolean(true),
            Value::Byte(255),
            Value::Int16(-1),
            Value::Int32(i32::MIN),
            Value::Int64(i64::MAX),
            Value::Single(Single::from_bits(0x8000_0000)),
            Value::Double(Double::from_bits(0x7ff8_0000_0000_1234)),
            Value::Currency(Currency::from_scaled(-12345)),
            Value::Decimal(Decimal::new(12300, 4).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
        ] {
            assert_eq!(cvar(value.clone()), value);
        }
    }

    #[test]
    fn owned_payloads_are_moved_without_copying() {
        let text = String::from("a\0b");
        let text_pointer = text.as_ptr();
        let Value::Text(text) = cvar(Value::Text(text)) else {
            unreachable!();
        };
        assert_eq!(text.as_ptr(), text_pointer);
        assert_eq!(text.as_bytes(), b"a\0b");

        let bytes = vec![0, 255, 127];
        let bytes_pointer = bytes.as_ptr();
        let Value::Binary(bytes) = cvar(Value::Binary(bytes)) else {
            unreachable!();
        };
        assert_eq!(bytes.as_ptr(), bytes_pointer);
        assert_eq!(bytes, [0, 255, 127]);
    }
}

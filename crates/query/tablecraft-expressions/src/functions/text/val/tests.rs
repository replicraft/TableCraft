use super::*;
use crate::functions::text::{TextFunction, apply};
use proptest::prelude::*;

struct Buffer {
    text: String,
    steps: usize,
    bytes: usize,
}
impl TextOutput for Buffer {
    fn step(&mut self) -> Result<(), EvalError> {
        self.steps = self.steps.checked_sub(1).ok_or(EvalError::Cancelled)?;
        Ok(())
    }
    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        self.step()?;
        if self.text.len() + text.len() > self.bytes {
            return Err(EvalError::TextLimit);
        }
        self.text.push_str(text);
        Ok(())
    }
}
impl NumericBuffer for Buffer {
    fn text(&self) -> &str {
        &self.text
    }
}
fn bounded(text: &str, steps: usize, bytes: usize) -> Result<f64, EvalError> {
    let args = [Value::Text(text.into())];
    let TextResult::NumericPrefix(prefix) = apply(TextFunction::Val, &args, || Ok(()))? else {
        unreachable!()
    };
    prefix
        .evaluate(&mut Buffer {
            text: String::new(),
            steps,
            bytes,
        })
        .map(Double::to_float)
}
fn parse(text: &str) -> Result<f64, EvalError> {
    bounded(text, 100_000, 65_536)
}

#[test]
fn documented_prefixes_and_explicit_whitespace_exponent_profile() {
    for (text, expected) in [
        ("2457", 2457.0),
        (" 2 45 7", 2457.0),
        ("24 and 57", 24.0),
        ("    1615 198th Street N.E.", 1615198.0),
        ("&HFFFF", -1.0),
        ("- . 5", -0.5),
        ("+1\t2\n3.5", 123.5),
        ("1,234", 1.0),
        ("$20", 0.0),
        ("１２", 0.0),
        (".", 0.0),
        ("", 0.0),
        ("12\r34", 12.0),
        ("12\0x", 12.0),
        ("2 D - 2x", 0.02),
        ("12e+oops", 12.0),
        ("12.5.4", 12.5),
        ("50%", 50.0),
        ("& H F F F F", -1.0),
        ("&O177777", -1.0),
        ("&H8000", -32768.0),
        ("&H10000", 65536.0),
        ("&HFFFF&", 65535.0),
        ("&HFFFFFFFF", -1.0),
        ("&HFFFFFFFF^", 4294967295.0),
        ("&HFFFFFFFFFFFFFFFF^", -1.0),
        ("&H", 0.0),
        ("&O8", 0.0),
        ("-32768%", -32768.0),
        ("9223372036854775807^", i64::MAX as f64),
        ("-9223372036854775808^", i64::MIN as f64),
        ("1.23445@", 1.2344),
        ("1.23455@", 1.2346),
        ("-1.23455@", -1.2346),
        ("0.00005@", 0.0),
        ("0.000051@", 0.0001),
        ("1e-99999@", 0.0),
        ("0e999999999999999999999%", 0.0),
        ("1.25!", 1.25),
    ] {
        assert_eq!(parse(text), Ok(expected), "{text:?}");
    }
    assert!(parse("-0").unwrap().is_sign_negative());
    assert_eq!(parse(&format!("{}1", "0".repeat(512))), Ok(1.0));
}
#[test]
fn suffix_errors_are_checked_before_double_rounding() {
    for text in ["50.5%", "1$", "0.1&", "1e-999999999999999999999%"] {
        assert_eq!(parse(text), Err(EvalError::TypeMismatch), "{text}");
    }
    for text in [
        "32768%",
        "2147483648&",
        "9223372036854775808^",
        "-9223372036854775809^",
        "922337203685477.5808@",
        "1e400",
        "1e40!",
        "&H100000000",
        "&H10000%",
        "&H10000000000000000^",
    ] {
        assert_eq!(parse(text), Err(EvalError::Overflow), "{text}");
    }
    assert_eq!(
        apply(TextFunction::Val, &[Value::Null], || Ok(())),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        apply(TextFunction::Val, &[Value::Int16(12)], || Ok(())),
        Err(EvalError::TypeMismatch)
    );
    assert!(apply(TextFunction::Val, &[], || Ok(())).is_err());
}
#[test]
fn normalization_obeys_work_bytes_and_cancellation() {
    assert_eq!(bounded("1234", 100, 3), Err(EvalError::TextLimit));
    assert_eq!(bounded("     1", 3, 10), Err(EvalError::Cancelled));
    assert_eq!(bounded("&HFFFF", 2, 0), Err(EvalError::Cancelled));
    assert_eq!(bounded("&HFFFF", 100, 0), Ok(-1.0));
    assert_eq!(bounded("1234", 100, 4), Ok(1234.0));
}
proptest! {
    #[test]
    fn exact_integer_suffixes_use_integer_reference(value in any::<i32>()) {
        prop_assert_eq!(parse(&format!("{value}&tail")), Ok(f64::from(value)));
    }
    #[test]
    fn radix_widths_match_independent_signed_reference(value in any::<u32>()) {
        prop_assert_eq!(parse(&format!("&H{value:X}&")), Ok(f64::from(value as i32)));
    }
}

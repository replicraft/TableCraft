use super::*;
use crate::functions::text::apply;
use proptest::prelude::*;
use std::slice;
use tablecraft_core::values::LegacyDateTime;
struct Output {
    text: String,
    steps: usize,
    bytes: usize,
}
impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        if self.steps == 0 {
            return Err(EvalError::Cancelled);
        }
        self.steps -= 1;
        Ok(())
    }
    fn append(&mut self, fragment: &str) -> Result<(), EvalError> {
        self.step()?;
        if self.text.len() + fragment.len() > self.bytes {
            return Err(EvalError::TextLimit);
        }
        self.text.push_str(fragment);
        Ok(())
    }
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn render(
    function: TextFunction,
    args: &[Value],
    steps: usize,
    bytes: usize,
) -> Result<Value, EvalError> {
    match apply(function, args, || Ok(()))? {
        TextResult::Null => Ok(Value::Null),
        TextResult::Construction(construction) => {
            let mut output = Output {
                text: String::new(),
                steps,
                bytes,
            };
            construction.write(&mut output)?;
            Ok(Value::Text(output.text))
        }
        _ => unreachable!(),
    }
}
#[test]
fn repeats_support_checked_counts_first_units_ascii_modulo_and_null() {
    for (function, args, expected) in [
        (TextFunction::Space, vec![Value::Int16(3)], text("   ")),
        (TextFunction::Space, vec![Value::Int16(0)], text("")),
        (TextFunction::Space, vec![Value::Null], Value::Null),
        (
            TextFunction::String,
            vec![Value::Int16(3), text("é中")],
            text("ééé"),
        ),
        (
            TextFunction::String,
            vec![Value::Int16(3), text("\0abc")],
            text("\0\0\0"),
        ),
        (
            TextFunction::String,
            vec![Value::Int16(5), Value::Int16(42)],
            text("*****"),
        ),
        (
            TextFunction::String,
            vec![Value::Int16(2), Value::Int16(298)],
            text("**"),
        ),
        (
            TextFunction::String,
            vec![Value::Int16(2), Value::Int16(256)],
            text("\0\0"),
        ),
        (
            TextFunction::String,
            vec![Value::Null, text("x")],
            Value::Null,
        ),
        (
            TextFunction::String,
            vec![Value::Int16(-1), Value::Null],
            Value::Null,
        ),
        (
            TextFunction::Space,
            vec![Value::Double(tablecraft_core::values::Double::from_float(
                2.5,
            ))],
            text("  "),
        ),
        (
            TextFunction::String,
            vec![
                Value::Int16(2),
                Value::Double(tablecraft_core::values::Double::from_float(65.5)),
            ],
            text("BB"),
        ),
    ] {
        assert_eq!(render(function, &args, 1000, 1000), Ok(expected));
    }
}
#[test]
fn reversal_uses_bmp_units_preserves_controls_and_rejects_unrepresentable_units() {
    for (input, expected) in [
        ("", ""),
        ("abc", "cba"),
        ("e\u{301}é中", "中é\u{301}e"),
        ("a\0b", "b\0a"),
        ("a\r\n", "\n\ra"),
    ] {
        assert_eq!(
            render(TextFunction::StrReverse, &[text(input)], 1000, 1000),
            Ok(text(expected))
        );
    }
    assert_eq!(
        render(TextFunction::StrReverse, &[text("😀x")], 1000, 1000),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        render(TextFunction::StrReverse, &[Value::Null], 1000, 1000),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        render(TextFunction::StrReverse, &[Value::Int16(1)], 1000, 1000),
        Err(EvalError::TypeMismatch)
    );
}
#[test]
fn domains_code_pages_and_huge_counts_fail_without_unbounded_output() {
    for (function, args, error) in [
        (
            TextFunction::Space,
            vec![Value::Int16(-1)],
            EvalError::Domain,
        ),
        (
            TextFunction::Space,
            vec![Value::Int64(i64::MAX)],
            EvalError::Overflow,
        ),
        (
            TextFunction::String,
            vec![Value::Int16(1), text("")],
            EvalError::Domain,
        ),
        (
            TextFunction::String,
            vec![Value::Int16(1), text("😀")],
            EvalError::UnrepresentableText,
        ),
        (
            TextFunction::String,
            vec![Value::Int16(1), Value::Int16(128)],
            EvalError::CharacterPolicyRequired,
        ),
        (
            TextFunction::String,
            vec![Value::Int16(1), Value::Int16(-1)],
            EvalError::Domain,
        ),
    ] {
        assert_eq!(render(function, &args, 1000, 1000), Err(error));
    }
    assert_eq!(
        render(TextFunction::Space, &[Value::Int32(i32::MAX)], 1000, 5),
        Err(EvalError::TextLimit)
    );
    assert_eq!(
        render(
            TextFunction::String,
            &[Value::Int32(i32::MAX), text("é")],
            1000,
            5
        ),
        Err(EvalError::TextLimit)
    );
    assert_eq!(
        render(TextFunction::Space, &[Value::Int16(100)], 3, 1000),
        Err(EvalError::Cancelled)
    );
    assert_eq!(
        render(TextFunction::StrReverse, &[text("abcdef")], 3, 1000),
        Err(EvalError::Cancelled)
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn repeated_bmp_units_match_independent_string_model(character in any::<char>().prop_filter("BMP unit",|ch|ch.len_utf16()==1), count in 0usize..100) {
        let input=character.to_string()+"ignored";
        let expected=character.to_string().repeat(count);
        prop_assert_eq!(render(TextFunction::String,&[Value::Int32(count as i32),text(&input)],1000,1000),Ok(text(&expected)));
    }
    #[test]
    fn bmp_reverse_matches_independent_reversed_utf16_buffer(characters in prop::collection::vec(any::<char>().prop_filter("BMP unit",|ch|ch.len_utf16()==1),0..100)) {
        let input:std::string::String=characters.into_iter().collect();
        let mut units:Vec<_>=input.encode_utf16().collect();units.reverse();
        let expected=std::string::String::from_utf16(&units).unwrap();
        prop_assert_eq!(render(TextFunction::StrReverse,&[text(&input)],1000,1000),Ok(text(&expected)));
    }
}

#[test]
fn repetition_count_and_numeric_character_accept_raw_legacy_date_coercion() {
    let count = Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap());
    assert_eq!(
        render(TextFunction::Space, slice::from_ref(&count), 100, 100),
        Ok(text("  "))
    );
    let character = Value::DateTime(LegacyDateTime::from_ole_days(65.5).unwrap());
    assert_eq!(
        render(TextFunction::String, &[count, character], 100, 100),
        Ok(text("BB"))
    );
}

use super::*;
use crate::functions::text::{apply, apply_with_context, code_page::CharacterCodePage};
fn value(
    function: TextFunction,
    value: Value,
    page: Option<u16>,
) -> Result<TextResult<'static>, EvalError> {
    let args = [value];
    let result = apply_with_context(
        function,
        &args,
        TextContext {
            character_code_page: page.map(|id| CharacterCodePage::from_windows_id(id).unwrap()),
            ..Default::default()
        },
        || Ok(()),
    )?;
    Ok(match result {
        TextResult::Character(character) => TextResult::Character(character),
        TextResult::CharacterCode(code) => TextResult::CharacterCode(code),
        _ => unreachable!(),
    })
}
#[test]
fn byte_codes_use_exact_pages_and_chr_does_not_apply_string_modulo() {
    assert_eq!(
        value(TextFunction::Chr, Value::Int16(65), None),
        Ok(TextResult::Character('A'))
    );
    assert_eq!(
        value(TextFunction::Chr, Value::Int16(128), Some(1252)),
        Ok(TextResult::Character('€'))
    );
    assert_eq!(
        value(TextFunction::Chr, Value::Int16(192), Some(1251)),
        Ok(TextResult::Character('А'))
    );
    assert_eq!(
        value(TextFunction::Asc, Value::Text("€x".into()), Some(1252)),
        Ok(TextResult::CharacterCode(128))
    );
    assert_eq!(
        value(TextFunction::Asc, Value::Text("А".into()), Some(1251)),
        Ok(TextResult::CharacterCode(192))
    );
    for code in [-1, 256, 65535] {
        assert_eq!(
            value(TextFunction::Chr, Value::Int32(code), Some(1252)),
            Err(EvalError::Domain)
        );
    }
    assert_eq!(
        value(TextFunction::Asc, Value::Text("中".into()), Some(1252)),
        Err(EvalError::UnmappableCharacter)
    );
    assert_eq!(
        value(TextFunction::Asc, Value::Text("é".into()), None),
        Err(EvalError::CharacterPolicyRequired)
    );
    assert_eq!(
        value(TextFunction::Chr, Value::Int16(170), Some(1253)),
        Err(EvalError::InvalidCharacterCode)
    );
}
#[test]
fn unicode_codes_use_signed_integer_units_without_losing_supplementary_input() {
    for (code, expected) in [
        (0, '\0'),
        (65, 'A'),
        (32767, '\u{7fff}'),
        (-32768, '\u{8000}'),
        (65535, '\u{ffff}'),
        (-1, '\u{ffff}'),
    ] {
        assert_eq!(
            value(TextFunction::ChrW, Value::Int32(code), None),
            Ok(TextResult::Character(expected))
        );
        assert_eq!(
            value(
                TextFunction::AscW,
                Value::Text(expected.to_string()),
                Some(1251)
            ),
            Ok(TextResult::CharacterCode(code as i16))
        );
    }
    assert_eq!(
        value(TextFunction::AscW, Value::Text("😀x".into()), None),
        Ok(TextResult::CharacterCode(-10179))
    );
    for code in [0xd800, 0xdfff] {
        assert_eq!(
            value(TextFunction::ChrW, Value::Int32(code), None),
            Err(EvalError::UnrepresentableText)
        );
    }
    for code in [-32769, 65536] {
        assert_eq!(
            value(TextFunction::ChrW, Value::Int32(code), None),
            Err(EvalError::Domain)
        );
    }
}
#[test]
fn null_empty_type_and_numeric_conversion_failures_are_distinct() {
    for function in [
        TextFunction::Chr,
        TextFunction::Asc,
        TextFunction::ChrW,
        TextFunction::AscW,
    ] {
        assert_eq!(
            value(function, Value::Null, None),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            apply(function, &[], || panic!("bad arity evaluated")),
            Err(EvalError::UnsupportedExpression)
        );
    }
    for function in [TextFunction::Asc, TextFunction::AscW] {
        assert_eq!(
            value(function, Value::Text("".into()), None),
            Err(EvalError::Domain)
        );
        assert_eq!(
            value(function, Value::Int16(1), None),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        value(
            TextFunction::Chr,
            Value::Double(tablecraft_core::values::Double::from_float(65.5)),
            None
        ),
        Ok(TextResult::Character('B'))
    );
    assert_eq!(
        value(TextFunction::ChrW, Value::Boolean(true), None),
        Ok(TextResult::Character('\u{ffff}'))
    );
}
#[test]
fn every_representable_unicode_unit_round_trips_to_an_independent_signed_model() {
    for unit in 0u32..=65535 {
        let Some(character) = char::from_u32(unit) else {
            continue;
        };
        let code = unit as u16 as i16;
        assert_eq!(
            value(TextFunction::AscW, Value::Text(character.to_string()), None),
            Ok(TextResult::CharacterCode(code))
        );
        assert_eq!(
            value(TextFunction::ChrW, Value::Int16(code), None),
            Ok(TextResult::Character(character))
        );
    }
}

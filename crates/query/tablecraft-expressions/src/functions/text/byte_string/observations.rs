//! Nonallocating UTF-16LE byte observations over representable Unicode Text.
use crate::EvalError;

const UTF16_UNIT_BYTES: i32 = size_of::<u16>() as i32;

pub(in crate::functions::text) fn length(
    text: &str,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<i32, EvalError> {
    super::super::units(text, step)?
        .checked_mul(UTF16_UNIT_BYTES)
        .ok_or(EvalError::Overflow)
}

pub(in crate::functions::text) fn first_byte(text: &str) -> Result<u8, EvalError> {
    let unit = text.encode_utf16().next().ok_or(EvalError::Domain)?;
    Ok(unit.to_le_bytes()[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::text::{
        TextContext, TextFunction, TextResult, apply_with_context, code_page::CharacterCodePage,
    };
    use proptest::prelude::*;
    use std::cell::Cell;
    use tablecraft_core::values::Value;

    #[test]
    fn unicode_byte_lengths_and_first_bytes_match_independent_goldens() {
        for (text, bytes, first) in [
            ("A", 2, 65),
            ("é", 2, 233),
            ("Ā", 2, 0),
            ("中", 2, 45),
            ("🦀", 4, 62),
            ("é中🦀", 8, 233),
            ("e\u{301}", 4, 101),
            ("\0", 2, 0),
        ] {
            assert_eq!(length(text, &mut || Ok(())), Ok(bytes));
            assert_eq!(first_byte(text), Ok(first));
        }
        assert_eq!(length("", &mut || Ok(())), Ok(0));
        assert_eq!(first_byte(""), Err(EvalError::Domain));
    }

    #[test]
    fn raw_byte_observation_ignores_ansi_pages_and_preserves_null_contracts() {
        let page = CharacterCodePage::from_windows_id(1252).unwrap();
        for context in [
            TextContext::default(),
            TextContext {
                character_code_page: Some(page),
                ..Default::default()
            },
        ] {
            assert_eq!(
                apply_with_context(
                    TextFunction::AscB,
                    &[Value::Text("中".into())],
                    context,
                    || Ok(())
                ),
                Ok(TextResult::CharacterCode(45))
            );
        }
        assert_eq!(
            apply_with_context(
                TextFunction::LenB,
                &[Value::Null],
                Default::default(),
                || Ok(())
            ),
            Ok(TextResult::Null)
        );
        assert_eq!(
            apply_with_context(
                TextFunction::AscB,
                &[Value::Null],
                Default::default(),
                || Ok(())
            ),
            Err(EvalError::InvalidNullConversion)
        );
        for function in [TextFunction::LenB, TextFunction::AscB] {
            assert_eq!(
                apply_with_context(
                    function,
                    &[Value::Binary(vec![65, 0])],
                    Default::default(),
                    || Ok(())
                ),
                Err(EvalError::TypeMismatch)
            );
        }
    }

    #[test]
    fn length_scanning_checks_each_observed_cancellation_point() {
        let text = "Aé中🦀";
        let count = Cell::new(0);
        for stop in 1..=text.chars().count() {
            count.set(0);
            assert_eq!(
                length(text, &mut || {
                    count.set(count.get() + 1);
                    if count.get() == stop {
                        Err(EvalError::Cancelled)
                    } else {
                        Ok(())
                    }
                }),
                Err(EvalError::Cancelled)
            );
        }
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]

        #[test]
        fn byte_observations_match_scalar_code_point_models(characters in prop::collection::vec(any::<char>(), 0..100)) {
            let text: String = characters.iter().collect();
            let expected_length: i32 = characters.iter().map(|character| if u32::from(*character) > 0xffff { 4 } else { 2 }).sum();
            prop_assert_eq!(length(&text, &mut || Ok(())), Ok(expected_length));

            if let Some(character) = characters.first() {
                let scalar = u32::from(*character);
                let leading = if scalar > 0xffff { 0xd800 + ((scalar - 0x10000) >> 10) } else { scalar };
                prop_assert_eq!(first_byte(&text), Ok((leading & 255) as u8));
            }
        }
    }
}

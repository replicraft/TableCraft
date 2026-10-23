//! Explicit byte-page and signed UTF-16 character functions.
use super::{TextContext, TextFunction, TextResult};
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

const MIN_SIGNED_UNIT: i32 = i16::MIN as i32;
const MAX_UNSIGNED_UNIT: i32 = u16::MAX as i32;

pub(crate) fn apply(
    function: TextFunction,
    value: &Value,
    context: TextContext<'_>,
) -> Result<TextResult<'static>, EvalError> {
    if matches!(value, Value::Null) {
        return Err(EvalError::InvalidNullConversion);
    }
    if matches!(
        function,
        TextFunction::Chr | TextFunction::ChrString | TextFunction::ChrW
    ) {
        let Value::Int32(code) = Conversion::Long.apply(value)? else {
            return Err(EvalError::TypeMismatch);
        };
        let character = if function == TextFunction::ChrW {
            if !(MIN_SIGNED_UNIT..=MAX_UNSIGNED_UNIT).contains(&code) {
                return Err(EvalError::Domain);
            }
            // The accepted negative half is the signed spelling of a UTF-16 unit.
            char::from_u32(u32::from(code as u16)).ok_or(EvalError::UnrepresentableText)?
        } else {
            let code = u8::try_from(code).map_err(|_| EvalError::Domain)?;
            match context.character_code_page {
                Some(page) => page.decode_byte(code)?,
                None if code.is_ascii() => char::from(code),
                None => return Err(EvalError::CharacterPolicyRequired),
            }
        };
        return Ok(TextResult::Character(character));
    }
    let Value::Text(text) = value else {
        return Err(EvalError::TypeMismatch);
    };
    if function == TextFunction::AscB {
        return super::byte_string::first_byte(text)
            .map(|byte| TextResult::CharacterCode(i16::from(byte)));
    }
    let character = text.chars().next().ok_or(EvalError::Domain)?;
    let code = if function == TextFunction::AscW {
        // Returning a numeric leading surrogate loses no input text; unlike ChrW,
        // this operation does not have to materialize an unpaired UTF-16 result.
        text.encode_utf16().next().ok_or(EvalError::Domain)? as i16
    } else {
        i16::from(match context.character_code_page {
            Some(page) => page.encode_character(character)?,
            None if character.is_ascii() => character as u8,
            None => return Err(EvalError::CharacterPolicyRequired),
        })
    };
    Ok(TextResult::CharacterCode(code))
}
#[cfg(test)]
mod tests;

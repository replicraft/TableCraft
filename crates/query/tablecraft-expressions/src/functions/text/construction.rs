//! Bounded Space/String construction and explicit BMP reversal.
use super::{TextFunction, TextResult, code_page::CharacterCodePage, output::TextOutput};
use crate::{
    EvalError,
    numeric::{IntegerType, coerce_integer},
};
use tablecraft_core::values::Value;

const CHARACTER_CODE_MODULUS: i32 = u8::MAX as i32 + 1;
const UTF8_CHARACTER_BYTES: usize = char::MAX.len_utf8();

/// Validated borrowed text-construction recipe; no intermediate buffer is allocated.
#[derive(Debug, PartialEq, Eq)]
pub struct Construction<'a> {
    operation: Operation<'a>,
}
#[derive(Debug, PartialEq, Eq)]
enum Operation<'a> {
    Repeat { character: char, count: usize },
    Reverse(&'a str),
}
pub(super) fn prepare(
    function: TextFunction,
    arguments: &[Value],
    code_page: Option<CharacterCodePage>,
) -> Result<TextResult<'_>, EvalError> {
    let operation = match function {
        TextFunction::Space | TextFunction::String => {
            // These Variant-returning functions preserve NULL, independently of
            // errors already raised while evaluating another written argument.
            if arguments.iter().any(|value| matches!(value, Value::Null)) {
                return Ok(TextResult::Null);
            }
            let count = long(&arguments[0])?;
            if count < 0 {
                return Err(EvalError::Domain);
            }
            let character = if function == TextFunction::Space {
                ' '
            } else {
                match &arguments[1] {
                    Value::Text(text) => {
                        let character = text.chars().next().ok_or(EvalError::Domain)?;
                        if character.len_utf16() != 1 {
                            return Err(EvalError::UnrepresentableText);
                        }
                        character
                    }
                    value => {
                        let code = long(value)?;
                        if code < 0 {
                            return Err(EvalError::Domain);
                        }
                        let code = (code % CHARACTER_CODE_MODULUS) as u8;
                        match code_page {
                            Some(page) => page.decode_byte(code)?,
                            None if code.is_ascii() => char::from(code),
                            None => return Err(EvalError::CharacterPolicyRequired),
                        }
                    }
                }
            };
            Operation::Repeat {
                character,
                count: count as usize,
            }
        }
        TextFunction::StrReverse => match &arguments[0] {
            Value::Text(text) => Operation::Reverse(text),
            Value::Null => return Err(EvalError::InvalidNullConversion),
            _ => return Err(EvalError::TypeMismatch),
        },
        _ => return Err(EvalError::UnsupportedExpression),
    };
    Ok(TextResult::Construction(Construction { operation }))
}
impl Construction<'_> {
    /// Stream repeated single-unit characters or reversed BMP units. Non-BMP
    /// reversal fails rather than replacing/reordering unpaired surrogates. The
    /// host must charge payloads before appending and discard partial output on error.
    pub fn write(&self, output: &mut impl TextOutput) -> Result<(), EvalError> {
        match self.operation {
            Operation::Repeat { character, count } => {
                let mut buffer = [0; UTF8_CHARACTER_BYTES];
                let fragment = character.encode_utf8(&mut buffer);
                for _ in 0..count {
                    output.step()?;
                    output.append(fragment)?;
                }
            }
            Operation::Reverse(text) => {
                for character in text.chars().rev() {
                    output.step()?;
                    if character.len_utf16() != 1 {
                        return Err(EvalError::UnrepresentableText);
                    }
                    let mut buffer = [0; UTF8_CHARACTER_BYTES];
                    output.append(character.encode_utf8(&mut buffer))?;
                }
            }
        }
        Ok(())
    }
}
fn long(value: &Value) -> Result<i32, EvalError> {
    match coerce_integer(value, IntegerType::Int32)? {
        Value::Int32(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests;

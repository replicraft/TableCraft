//! Explicit StrConv conversion flags and strict single-byte encoding kernels.
mod flags;
mod kana;
mod locale;

use super::{
    code_page::CharacterCodePage,
    output::{ByteOutput, TextOutput},
};
use crate::EvalError;
#[cfg(feature = "sql")]
pub(crate) use flags::Operation;
pub use flags::{CaseConversion, Conversion, EncodingConversion};
pub use kana::KanaConversion;
pub use locale::LocaleProfile;

/// Convert representable Unicode characters to exact single-byte string data.
/// Missing page selection permits ASCII only. No replacement or best-fit mapping.
pub fn encode_ansi(
    text: &str,
    page: Option<CharacterCodePage>,
    output: &mut impl ByteOutput,
) -> Result<(), EvalError> {
    output.step()?;
    for character in text.chars() {
        output.step()?;
        let byte = match page {
            Some(page) => page.encode_character(character)?,
            None if character.is_ascii() => character as u8,
            None => return Err(EvalError::CharacterPolicyRequired),
        };
        output.append(&[byte])?;
    }
    Ok(())
}

/// Decode exact single-byte string/array data to Unicode using the selected page.
/// The host meters output and discards any prefix when an input byte is unmapped.
pub fn decode_ansi(
    bytes: &[u8],
    page: Option<CharacterCodePage>,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    output.step()?;
    let mut buffer = [0; char::MAX.len_utf8()];
    for byte in bytes {
        output.step()?;
        let character = match page {
            Some(page) => page.decode_byte(*byte)?,
            None if byte.is_ascii() => char::from(*byte),
            None => return Err(EvalError::CharacterPolicyRequired),
        };
        output.append(character.encode_utf8(&mut buffer))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

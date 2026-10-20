//! Explicit supported single-byte Windows/OEM character-code policies.
use crate::EvalError;
use encoding_rs::{DecoderResult, EncoderResult, Encoding};

const OEM_CYRILLIC: u16 = 866;
const WINDOWS_THAI: u16 = 874;
const WINDOWS_CENTRAL_EUROPEAN: u16 = 1250;
const WINDOWS_CYRILLIC: u16 = 1251;
const WINDOWS_WESTERN_EUROPEAN: u16 = 1252;
const WINDOWS_GREEK: u16 = 1253;
const WINDOWS_TURKISH: u16 = 1254;
const WINDOWS_HEBREW: u16 = 1255;
const WINDOWS_ARABIC: u16 = 1256;
const WINDOWS_BALTIC: u16 = 1257;
const WINDOWS_VIETNAMESE: u16 = 1258;

/// A validated caller-selected code page. Uses encoding_rs/WHATWG mappings, not
/// an inferred system ANSI page or proof of native Access mapping equivalence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterCodePage(u16);
impl CharacterCodePage {
    /// Select Windows 874, 1250–1258, or OEM 866. Other pages (including system
    /// aliases, multibyte pages and Unicode encodings) are explicitly unsupported.
    pub fn from_windows_id(id: u16) -> Result<Self, EvalError> {
        let page = Self(id);
        page.encoding()?;
        Ok(page)
    }
    /// The explicitly supplied numeric Windows/OEM code-page identifier.
    pub const fn windows_id(self) -> u16 {
        self.0
    }
    /// Decode exactly one byte into one BMP character, without allocation, BOM
    /// sniffing, replacement characters or locale access. Unmapped bytes fail.
    pub fn decode_byte(self, byte: u8) -> Result<char, EvalError> {
        let mut output = [0u16; char::MAX.len_utf16()];
        let (result, read, written) = self
            .encoding()?
            .new_decoder_without_bom_handling()
            .decode_to_utf16_without_replacement(&[byte], &mut output, true);
        if result != DecoderResult::InputEmpty || read != 1 || written != 1 {
            return Err(EvalError::InvalidCharacterCode);
        }
        char::from_u32(u32::from(output[0])).ok_or(EvalError::InvalidCharacterCode)
    }
    /// Encode one character to exactly one byte, without best-fit/replacement or
    /// allocation. A decoded round trip must retain the original character.
    pub fn encode_character(self, character: char) -> Result<u8, EvalError> {
        let mut input = [0u16; char::MAX.len_utf16()];
        let input = character.encode_utf16(&mut input);
        let mut output = [0u8; char::MAX.len_utf8()];
        let (result, read, written) = self
            .encoding()?
            .new_encoder()
            .encode_from_utf16_without_replacement(input, &mut output, true);
        if result != EncoderResult::InputEmpty
            || read != input.len()
            || written != 1
            || self.decode_byte(output[0])? != character
        {
            return Err(EvalError::UnmappableCharacter);
        }
        Ok(output[0])
    }
    fn encoding(self) -> Result<&'static Encoding, EvalError> {
        Ok(match self.0 {
            OEM_CYRILLIC => encoding_rs::IBM866,
            WINDOWS_THAI => encoding_rs::WINDOWS_874,
            WINDOWS_CENTRAL_EUROPEAN => encoding_rs::WINDOWS_1250,
            WINDOWS_CYRILLIC => encoding_rs::WINDOWS_1251,
            WINDOWS_WESTERN_EUROPEAN => encoding_rs::WINDOWS_1252,
            WINDOWS_GREEK => encoding_rs::WINDOWS_1253,
            WINDOWS_TURKISH => encoding_rs::WINDOWS_1254,
            WINDOWS_HEBREW => encoding_rs::WINDOWS_1255,
            WINDOWS_ARABIC => encoding_rs::WINDOWS_1256,
            WINDOWS_BALTIC => encoding_rs::WINDOWS_1257,
            WINDOWS_VIETNAMESE => encoding_rs::WINDOWS_1258,
            _ => return Err(EvalError::UnsupportedCharacterCodePage),
        })
    }
}
#[cfg(test)]
mod tests;

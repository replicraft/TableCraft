//! Complete string encodings, separate from one-byte character-code policies.
use super::super::{
    code_page::CharacterCodePage,
    output::{ByteOutput, TextOutput},
};
use crate::EvalError;
use encoding_rs::{DecoderResult, EncoderResult, SHIFT_JIS};

const JAPANESE_PAGE: u16 = 932;
const MAX_ENCODED_BYTES: usize = char::MAX.len_utf8();
const MAX_UTF16_UNITS: usize = char::MAX.len_utf16();
const FIRST_LEAD_START: u8 = 0x81;
const FIRST_LEAD_END: u8 = 0x9f;
const SECOND_LEAD_START: u8 = 0xe0;
const SECOND_LEAD_END: u8 = 0xfc;
const SINGLE_BYTE: usize = size_of::<u8>();
const DOUBLE_BYTE: usize = 2 * SINGLE_BYTE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Single(CharacterCodePage),
    Japanese,
}

/// A complete explicit string codec. Japanese 932 selects encoding_rs/WHATWG
/// Shift_JIS with strict Unicode encode round trips, not Windows NLS or best-fit
/// behavior. This type does not enable packed DBCS Chr/Asc character codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringCodePage(Page);

impl StringCodePage {
    /// Select Japanese 932 or an existing supported single-byte page.
    /// System aliases, Unicode encodings and other multibyte pages refuse.
    pub fn from_windows_id(id: u16) -> Result<Self, EvalError> {
        if id == JAPANESE_PAGE {
            return Ok(Self(Page::Japanese));
        }
        CharacterCodePage::from_windows_id(id).map(Self::from)
    }

    /// Exact caller-selected page identity, without ambient locale discovery.
    pub const fn windows_id(self) -> u16 {
        match self.0 {
            Page::Single(page) => page.windows_id(),
            Page::Japanese => JAPANESE_PAGE,
        }
    }

    /// Encode exact Unicode through a metered sink, without substitution or
    /// best-fit aliases. Discard the output prefix after any error.
    pub fn encode(self, text: &str, output: &mut impl ByteOutput) -> Result<(), EvalError> {
        match self.0 {
            Page::Single(page) => super::encode_ansi(text, Some(page), output),
            Page::Japanese => encode_japanese(text, output),
        }
    }

    /// Decode complete byte sequences without BOM sniffing or replacement.
    /// Duplicate Japanese byte spellings may produce the same Unicode scalar;
    /// this does not promise identity after a subsequent canonical encoding.
    pub fn decode(self, bytes: &[u8], output: &mut impl TextOutput) -> Result<(), EvalError> {
        match self.0 {
            Page::Single(page) => super::decode_ansi(bytes, Some(page), output),
            Page::Japanese => decode_japanese(bytes, output),
        }
    }
}

impl From<CharacterCodePage> for StringCodePage {
    fn from(page: CharacterCodePage) -> Self {
        Self(Page::Single(page))
    }
}

fn decode_token(bytes: &[u8]) -> Result<char, EvalError> {
    let mut units = [0; MAX_UTF16_UNITS];
    let (result, read, written) = SHIFT_JIS
        .new_decoder_without_bom_handling()
        .decode_to_utf16_without_replacement(bytes, &mut units, true);
    if result != DecoderResult::InputEmpty || read != bytes.len() || written != 1 {
        return Err(EvalError::InvalidCharacterCode);
    }
    char::from_u32(u32::from(units[0])).ok_or(EvalError::InvalidCharacterCode)
}

fn encode_japanese(text: &str, output: &mut impl ByteOutput) -> Result<(), EvalError> {
    output.step()?;
    for character in text.chars() {
        output.step()?;
        let mut units = [0; MAX_UTF16_UNITS];
        let input = character.encode_utf16(&mut units);
        let mut bytes = [0; MAX_ENCODED_BYTES];
        let (result, read, written) = SHIFT_JIS
            .new_encoder()
            .encode_from_utf16_without_replacement(input, &mut bytes, true);
        if result != EncoderResult::InputEmpty
            || read != input.len()
            || !(SINGLE_BYTE..=DOUBLE_BYTE).contains(&written)
        {
            return Err(EvalError::UnmappableCharacter);
        }
        // The underlying non-replacing encoder still accepts yen, overline and
        // minus aliases whose decoded scalars differ. Preserve exact Unicode.
        if decode_token(&bytes[..written])? != character {
            return Err(EvalError::UnmappableCharacter);
        }
        output.append(&bytes[..written])?;
    }
    Ok(())
}

fn decode_japanese(bytes: &[u8], output: &mut impl TextOutput) -> Result<(), EvalError> {
    output.step()?;
    let mut position = 0;
    let mut text = [0; char::MAX.len_utf8()];
    while position < bytes.len() {
        output.step()?;
        let lead = bytes[position];
        let width = if (FIRST_LEAD_START..=FIRST_LEAD_END).contains(&lead)
            || (SECOND_LEAD_START..=SECOND_LEAD_END).contains(&lead)
        {
            DOUBLE_BYTE
        } else {
            SINGLE_BYTE
        };
        let end = position
            .checked_add(width)
            .ok_or(EvalError::InvalidCharacterCode)?;
        let token = bytes
            .get(position..end)
            .ok_or(EvalError::InvalidCharacterCode)?;
        let character = decode_token(token)?;
        output.append(character.encode_utf8(&mut text))?;
        position = end;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

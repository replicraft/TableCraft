//! Strict XML 1.0 mandatory encodings; package bytes and ZIP names stay unchanged.
mod prefix;
pub(super) use prefix::PrefixReader;

use super::{TemplateError, check};
use crate::ReadError;
use quick_xml::events::BytesDecl;
use std::borrow::Cow;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const MAX_XML_BYTES: usize = 8 * 1024 * 1024;
const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];
const LITTLE_BOM: &[u8] = &[0xff, 0xfe];
const BIG_BOM: &[u8] = &[0xfe, 0xff];
const HIGH_SURROGATE_START: u16 = 0xd800;
const HIGH_SURROGATE_END: u16 = 0xdbff;
const LOW_SURROGATE_START: u16 = 0xdc00;
const LOW_SURROGATE_END: u16 = 0xdfff;
const SURROGATE_BITS: u32 = 10;
const SUPPLEMENTARY_START: u32 = 0x10000;

#[derive(Clone, Copy)]
pub(super) enum Encoding {
    Utf8,
    Little,
    Big,
}

pub(super) struct Decoded<'a> {
    pub text: Cow<'a, str>,
    pub encoding: Encoding,
}

pub(super) fn detect(bytes: &[u8]) -> (Encoding, usize) {
    if bytes.starts_with(LITTLE_BOM) {
        (Encoding::Little, LITTLE_BOM.len())
    } else if bytes.starts_with(BIG_BOM) {
        (Encoding::Big, BIG_BOM.len())
    } else if bytes.starts_with(UTF8_BOM) {
        (Encoding::Utf8, UTF8_BOM.len())
    } else {
        (Encoding::Utf8, 0)
    }
}

fn word(bytes: &[u8], position: usize, encoding: Encoding) -> Result<u16, TemplateError> {
    let value = bytes
        .get(position..position + size_of::<u16>())
        .ok_or(TemplateError::Malformed)?
        .try_into()
        .unwrap();
    Ok(match encoding {
        Encoding::Little => u16::from_le_bytes(value),
        Encoding::Big => u16::from_be_bytes(value),
        Encoding::Utf8 => return Err(TemplateError::Malformed),
    })
}

pub(super) fn next(
    bytes: &[u8],
    position: usize,
    encoding: Encoding,
) -> Result<(char, usize), TemplateError> {
    let (character, end) = match encoding {
        Encoding::Utf8 => {
            let first = *bytes.get(position).ok_or(TemplateError::Malformed)?;
            let width = match first {
                0..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => return Err(TemplateError::Malformed),
            };
            let end = position + width;
            let scalar =
                std::str::from_utf8(bytes.get(position..end).ok_or(TemplateError::Malformed)?)
                    .map_err(|_| TemplateError::Malformed)?;
            (scalar.chars().next().ok_or(TemplateError::Malformed)?, end)
        }
        Encoding::Little | Encoding::Big => {
            let first = word(bytes, position, encoding)?;
            let mut end = position + size_of::<u16>();
            let codepoint = if (HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&first) {
                let second = word(bytes, end, encoding)?;
                if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&second) {
                    return Err(TemplateError::Malformed);
                }
                end += size_of::<u16>();
                SUPPLEMENTARY_START
                    + (u32::from(first - HIGH_SURROGATE_START) << SURROGATE_BITS)
                    + u32::from(second - LOW_SURROGATE_START)
            } else {
                u32::from(first)
            };
            (
                char::from_u32(codepoint).ok_or(TemplateError::Malformed)?,
                end,
            )
        }
    };
    if !matches!(character, '\t' | '\r' | '\n')
        && (character < ' ' || matches!(character, '\u{fffe}' | '\u{ffff}'))
    {
        return Err(TemplateError::Malformed);
    }
    Ok((character, end))
}

pub(super) fn decode<'a>(
    bytes: &'a [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Decoded<'a>, TemplateError> {
    if bytes.len() > MAX_XML_BYTES {
        return Err(TemplateError::Unsupported);
    }
    let (encoding, start) = detect(bytes);
    let mut position = start;
    let mut length = 0usize;
    while position < bytes.len() {
        check(budget, cancelled)?;
        let (character, end) = next(bytes, position, encoding)?;
        budget
            .charge(Resource::Steps, (end - position) as u64)
            .map_err(ReadError::from)?;
        length = length
            .checked_add(character.len_utf8())
            .filter(|length| *length <= MAX_XML_BYTES)
            .ok_or(TemplateError::Unsupported)?;
        position = end;
    }
    let text = if matches!(encoding, Encoding::Utf8) {
        Cow::Borrowed(std::str::from_utf8(&bytes[start..]).map_err(|_| TemplateError::Malformed)?)
    } else {
        budget
            .charge(Resource::Bytes, length as u64)
            .map_err(ReadError::from)?;
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| ReadError::Allocation)?;
        position = start;
        while position < bytes.len() {
            check(budget, cancelled)?;
            let (character, end) = next(bytes, position, encoding)?;
            text.push(character);
            position = end;
        }
        Cow::Owned(text)
    };
    check(budget, cancelled)?;
    Ok(Decoded { text, encoding })
}

pub(super) fn declaration(value: &BytesDecl<'_>, encoding: Encoding) -> Result<(), TemplateError> {
    if value
        .version()
        .map_err(|_| TemplateError::Malformed)?
        .as_ref()
        != "1.0"
    {
        return Err(TemplateError::Unsupported);
    }
    let Some(label) = value
        .encoding()
        .transpose()
        .map_err(|_| TemplateError::Malformed)?
    else {
        return Ok(());
    };
    let utf8 = label.eq_ignore_ascii_case("UTF-8");
    let utf16 = label.eq_ignore_ascii_case("UTF-16");
    if !utf8 && !utf16 {
        return Err(TemplateError::Unsupported);
    }
    if matches!(encoding, Encoding::Utf8) != utf8 {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

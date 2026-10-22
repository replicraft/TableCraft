//! Single-pass XML name decoding; escaped underscores are not decoded twice.
use super::TemplateError;

pub(super) fn decode(value: &str) -> Result<String, TemplateError> {
    let mut units = Vec::new();
    units
        .try_reserve(value.len())
        .map_err(|_| tablecraft_ace::InspectError::Allocation)?;
    let mut rest = value;
    while !rest.is_empty() {
        if rest.starts_with("_x")
            && rest.as_bytes().get(6) == Some(&b'_')
            && rest.as_bytes()[2..6].iter().all(u8::is_ascii_hexdigit)
        {
            units.push(u16::from_str_radix(&rest[2..6], 16).map_err(|_| TemplateError::Malformed)?);
            rest = &rest[7..];
        } else {
            let character = rest.chars().next().ok_or(TemplateError::Malformed)?;
            units.extend_from_slice(character.encode_utf16(&mut [0; 2]));
            rest = &rest[character.len_utf8()..];
        }
    }
    String::from_utf16(&units).map_err(|_| TemplateError::Unsupported)
}

//! OPC identifiers are IRIs, never filesystem paths or network destinations.
use super::{MAX_NAME_BYTES, TemplateError, check};
use iri_string::{
    format::ToStringFallible,
    types::{IriAbsoluteStr, IriReferenceStr},
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

const BASE_PREFIX: &str = "tcraft:/";
// Input decoding, base construction, resolver output and validated result.
const RESOLUTION_SCRATCH_COPIES: usize = 4;

/// Decode UTF-8 escapes for the physical ZIP-to-logical part mapping. ASCII
/// reserved escapes remain opaque; encoded unreserved ASCII is invalid in names.
pub(super) fn part(value: &str) -> Result<String, TemplateError> {
    let value = decode(value, false)?;
    validate(&value)?;
    Ok(value)
}

pub(super) fn zip_name(value: &str) -> Result<String, TemplateError> {
    // The Media Types stream is a physical control item, outside part grammar.
    if value == "[Content_Types].xml" {
        return Ok(value.into());
    }
    part(value)
}

fn validate(value: &str) -> Result<(), TemplateError> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES {
        return Err(TemplateError::Unsupported);
    }
    let absolute_path = format!("/{value}");
    let reference = IriReferenceStr::new(&absolute_path).map_err(|_| TemplateError::Unsupported)?;
    if reference.authority_str().is_some()
        || reference.query_str().is_some()
        || reference.fragment_str().is_some()
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment.ends_with('.'))
    {
        return Err(TemplateError::Unsupported);
    }
    Ok(())
}

pub(super) fn resolve(
    source: &str,
    target: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, TemplateError> {
    check(budget, cancelled)?;
    if source.len() > MAX_NAME_BYTES || target.len() > MAX_NAME_BYTES {
        return Err(TemplateError::Unsupported);
    }
    budget
        .charge(
            Resource::Bytes,
            ((source.len() + target.len() + BASE_PREFIX.len()) * RESOLUTION_SCRATCH_COPIES) as u64,
        )
        .map_err(InspectError::from)?;
    // Normalize unreserved escapes before traversal checks, never reserved ones.
    let target = decode(target, true)?;
    let reference = IriReferenceStr::new(&target).map_err(|_| TemplateError::Unsupported)?;
    if reference.scheme_str().is_some()
        || reference.authority_str().is_some()
        || reference.query_str().is_some()
        || reference.fragment_str().is_some()
    {
        return Err(TemplateError::Unsupported);
    }
    // Retain the reader's explicit refusal of references above the package root.
    // RFC resolution itself would clamp these; do not silently redirect them.
    let mut depth = if target.starts_with('/') {
        0
    } else {
        source.bytes().filter(|byte| *byte == b'/').count()
    };
    for segment in reference.path_str().split('/') {
        match segment {
            "" | "." => {}
            ".." => depth = depth.checked_sub(1).ok_or(TemplateError::Unsupported)?,
            _ => depth += 1,
        }
    }
    let base = format!("{BASE_PREFIX}{source}");
    let base = IriAbsoluteStr::new(&base).map_err(|_| TemplateError::Unsupported)?;
    let resolved = reference.resolve_against(base).and_normalize();
    resolved
        .ensure_rfc3986_normalizable()
        .map_err(|_| TemplateError::Unsupported)?;
    let resolved = resolved
        .try_to_string()
        .map_err(|_| InspectError::Allocation)?;
    let path = resolved
        .strip_prefix(BASE_PREFIX)
        .ok_or(TemplateError::Unsupported)?;
    let result = part(path)?;
    check(budget, cancelled)?;
    Ok(result)
}

fn decode(value: &str, unreserved: bool) -> Result<String, TemplateError> {
    if value.len() > MAX_NAME_BYTES {
        return Err(TemplateError::Unsupported);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| InspectError::Allocation)?;
    let bytes = value.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        if bytes[position] != b'%' {
            output.push(bytes[position]);
            position += 1;
            continue;
        }
        let escape = bytes
            .get(position + 1..position + 3)
            .ok_or(TemplateError::Unsupported)?;
        let digit = |byte: u8| {
            (byte as char)
                .to_digit(16)
                .ok_or(TemplateError::Unsupported)
        };
        let byte = ((digit(escape[0])? << 4) | digit(escape[1])?) as u8;
        if matches!(byte, b'/' | b'\\') {
            return Err(TemplateError::Unsupported);
        }
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        if is_unreserved && !unreserved {
            return Err(TemplateError::Unsupported);
        }
        if byte >= 0x80 || is_unreserved {
            output.push(byte);
        } else {
            output.extend_from_slice(&bytes[position..position + 3]);
        }
        position += 3;
    }
    String::from_utf8(output).map_err(|_| TemplateError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_names_decode_utf8_but_keep_reserved_escapes_opaque() {
        for (stored, logical) in [
            ("tables/caf%C3%A9.xml", "tables/café.xml"),
            ("tables/%F0%9F%A6%80.xml", "tables/🦀.xml"),
            ("tables/a%20b.xml", "tables/a%20b.xml"),
            ("tables/a+b.xml", "tables/a+b.xml"),
            ("tables/a%2Bb.xml", "tables/a%2Bb.xml"),
            ("tables/%252e.xml", "tables/%252e.xml"),
            ("tables/a:b.xml", "tables/a:b.xml"),
        ] {
            assert_eq!(part(stored).unwrap(), logical);
        }
        for invalid in [
            "a b.xml",
            "a\\b.xml",
            "a%2fb.xml",
            "a%5Cb.xml",
            "a%41.xml",
            "a%2e.xml",
            "a%",
            "a%GG.xml",
            "a%C3.xml",
            "a%FF.xml",
            "a%80.xml",
            "a%ED%A0%80.xml",
            "a%F4%90%80%80.xml",
            "a//b.xml",
            "a/..",
            "a./b.xml",
            "a?b.xml",
            "a#b.xml",
            "[trash]/a.xml",
            "/a.xml",
        ] {
            assert!(part(invalid).is_err(), "{invalid}");
        }
    }

    fn target(source: &str, target: &str) -> Result<String, TemplateError> {
        resolve(
            source,
            target,
            &mut ResourceBudget::new(100000, 0, 10000),
            &|| false,
        )
    }

    #[test]
    fn references_resolve_with_rfc_rules_and_package_root_bounds() {
        for (source, reference, expected) in [
            (
                "template/main.xml",
                "../tables/caf%C3%A9.xml",
                "tables/café.xml",
            ),
            ("template/main.xml", "/tables/%41.xml", "tables/A.xml"),
            ("template/main.xml", "%2e%2e/tables/a.xml", "tables/a.xml"),
            ("template/main.xml", "./sub/../a.xml", "template/a.xml"),
            (
                "template/main.xml",
                "../tables/%252e.xml",
                "tables/%252e.xml",
            ),
            ("template/main.xml", "/tables/a:b.xml", "tables/a:b.xml"),
            (
                "template/main.xml",
                "../tables/a%20b.xml",
                "tables/a%20b.xml",
            ),
        ] {
            assert_eq!(target(source, reference).unwrap(), expected);
        }
        for invalid in [
            "../../escape.xml",
            "%2e%2e/%2e%2e/escape.xml",
            "//server/a.xml",
            "https://example.invalid/a.xml",
            "a:b.xml",
            "../a%2fb.xml",
            "../a%5cb.xml",
            "a.xml?query",
            "a.xml#fragment",
            "a b.xml",
            "a//b.xml",
            "a./b.xml",
        ] {
            assert!(target("template/main.xml", invalid).is_err(), "{invalid}");
        }
    }
}

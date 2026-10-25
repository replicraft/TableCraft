use super::*;
use std::{cell::Cell, io::BufRead};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn wide(text: &str, little: bool) -> Vec<u8> {
    let mut bytes = if little {
        vec![0xff, 0xfe]
    } else {
        vec![0xfe, 0xff]
    };
    for unit in text.encode_utf16() {
        bytes.extend(if little {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    bytes
}

#[test]
fn mandatory_encodings_preserve_exact_unicode_and_remove_only_the_signature() {
    let text = "<x>é中🦀e\u{301} &amp; literal</x>";
    for bytes in [
        text.as_bytes().to_vec(),
        [vec![0xef, 0xbb, 0xbf], text.as_bytes().to_vec()].concat(),
        wide(text, true),
        wide(text, false),
    ] {
        assert_eq!(decode(&bytes, &mut budget(), &|| false).unwrap().text, text);
    }
    assert_eq!(
        decode(&wide("\u{feff}<x/>", true), &mut budget(), &|| false)
            .unwrap()
            .text,
        "\u{feff}<x/>"
    );
}

#[test]
fn declarations_match_original_encoding_and_do_not_enable_other_charsets_or_xml_versions() {
    for (encoding, matching, mismatching) in [
        (Encoding::Utf8, "UTF-8", "UTF-16"),
        (Encoding::Little, "uTf-16", "UTF-8"),
        (Encoding::Big, "UTF-16", "UTF-8"),
    ] {
        assert!(declaration(&BytesDecl::new("1.0", Some(matching), None), encoding).is_ok());
        assert_eq!(
            declaration(&BytesDecl::new("1.0", Some(mismatching), None), encoding),
            Err(TemplateError::Malformed)
        );
        for label in ["UTF-16LE", "UTF-16BE", "ISO-8859-1", "UTF-32"] {
            assert_eq!(
                declaration(&BytesDecl::new("1.0", Some(label), None), encoding),
                Err(TemplateError::Unsupported)
            );
        }
        assert!(declaration(&BytesDecl::new("1.0", None, None), encoding).is_ok());
        assert_eq!(
            declaration(&BytesDecl::new("1.1", None, None), encoding),
            Err(TemplateError::Unsupported)
        );
    }
}

#[test]
fn illegal_units_scalars_and_bomless_wide_inputs_never_use_replacement_characters() {
    for bytes in [
        vec![0xff, 0xfe, 0x00, 0xd8],
        vec![0xff, 0xfe, 0x00, 0xdc],
        vec![0xfe, 0xff, 0xd8, 0x00, 0x00, 0x41],
        vec![0xfe, 0xff, 0x00],
        vec![0xc0, 0x80],
        vec![0xed, 0xa0, 0x80],
        wide("<x>\0</x>", true),
        wide("<x>\u{ffff}</x>", false),
        wide("<x/>", true)[2..].to_vec(),
    ] {
        assert!(matches!(
            decode(&bytes, &mut budget(), &|| false),
            Err(TemplateError::Malformed)
        ));
    }
}

#[test]
fn encoded_and_decoded_byte_ceilings_are_independent() {
    let oversized = vec![b' '; 8 * 1024 * 1024 + 1];
    assert!(matches!(
        decode(&oversized, &mut budget(), &|| false),
        Err(TemplateError::Unsupported)
    ));

    // This original stream fits the encoded limit but its UTF-8 output does not.
    let text = "中".repeat((8 * 1024 * 1024) / 3 + 1);
    let bytes = wide(&text, true);
    assert!(bytes.len() < 8 * 1024 * 1024);
    assert!(matches!(
        decode(&bytes, &mut budget(), &|| false),
        Err(TemplateError::Unsupported)
    ));
}

#[test]
fn conversion_prepays_allocation_and_checks_work_and_cancellation() {
    let bytes = wide("<x>é🦀</x>", true);
    let mut no_bytes = ResourceBudget::new(0, 0, 100_000);
    assert!(
        matches!(decode(&bytes, &mut no_bytes, &|| false), Err(TemplateError::Read(InspectError::Limit(limit))) if limit.resource == Resource::Bytes)
    );
    let mut no_steps = ResourceBudget::new(100_000, 0, 0);
    assert!(
        matches!(decode(&bytes, &mut no_steps, &|| false), Err(TemplateError::Read(InspectError::Limit(limit))) if limit.resource == Resource::Steps)
    );
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 3
    };
    assert!(matches!(
        decode(&bytes, &mut budget(), &cancelled),
        Err(TemplateError::Read(InspectError::Cancelled))
    ));
}

#[test]
fn prefix_raw_position_tracks_consumption_instead_of_prefetch() {
    let bytes = wide("🦀x", true);
    let mut pool = budget();
    let mut reader = PrefixReader::new(&bytes, &mut pool, &|| false);
    assert_eq!(reader.position(), 2);
    assert_eq!(reader.fill_buf().unwrap(), "🦀".as_bytes());
    assert_eq!(reader.position(), 2);
    reader.consume(1);
    assert_eq!(reader.position(), 2);
    reader.consume(3);
    assert_eq!(reader.position(), 6);
    assert_eq!(reader.fill_buf().unwrap(), b"x");
    assert_eq!(reader.position(), 6);
    reader.consume(1);
    assert_eq!(reader.position(), 8);
}

#[test]
fn prefix_preserves_first_typed_failure_and_never_refills_allowances() {
    let bytes = wide("<x/>", true);
    let mut pool = ResourceBudget::new(0, 0, 100_000);
    let mut reader = PrefixReader::new(&bytes, &mut pool, &|| false);
    assert!(reader.fill_buf().is_err());
    assert!(reader.fill_buf().is_err());
    assert!(
        matches!(reader.take_error(), TemplateError::Read(InspectError::Limit(limit)) if limit.resource == Resource::Bytes)
    );
    assert_eq!(reader.position(), 2);
}

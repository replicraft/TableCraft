use super::*;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};

fn page(code: u8, encoded: bool) -> Vec<u8> {
    let mut page = vec![0; crate::format::page::PAGE_BYTES];
    page[..4].copy_from_slice(&[0, 1, 0, 0]);
    page[4..20].copy_from_slice(if code <= 1 {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    });
    page[20] = code;
    page[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    if encoded {
        page[62] ^= 1;
    }
    page
}

fn envelope(page: &mut [u8], major: u16, minor: u16, tail: &[u8]) {
    let bytes = VERSION_BYTES + tail.len();
    page[OFFICE_INFO_LENGTH_OFFSET..OFFICE_INFO_OFFSET]
        .copy_from_slice(&(bytes as u16).to_le_bytes());
    page[OFFICE_INFO_OFFSET..OFFICE_INFO_OFFSET + 2].copy_from_slice(&major.to_le_bytes());
    page[OFFICE_INFO_OFFSET + 2..OFFICE_INFO_OFFSET + VERSION_BYTES]
        .copy_from_slice(&minor.to_le_bytes());
    page[OFFICE_INFO_OFFSET + VERSION_BYTES..OFFICE_INFO_OFFSET + bytes].copy_from_slice(tail);
}

fn source(page: &[u8]) -> Cursor<Vec<u8>> {
    let mut data = vec![0; page.len() * 3];
    data[..page.len()].copy_from_slice(page);
    Cursor::new(data)
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(8192, 0, 100)
}

#[test]
fn envelopes_are_independent_of_ace_file_version_and_do_not_grant_reads() {
    for code in [2, 3, 5, 6] {
        let mut p = page(code, true);
        envelope(&mut p, 4, 4, &[0x40, 0, 0, 0, b'<']);
        let info = parse_office(&p).unwrap();
        assert_eq!(info.version(), (4, 4));
        assert_eq!(info.family(), OfficeEncryptionFamily::Agile);
        assert_eq!(info.descriptor_bytes(), 9);
        assert_eq!(info.flags(), None);
        assert_eq!(
            inspect_encryption(&mut source(&p), &mut budget(), &|| false),
            Ok(EncryptionInspection::Office(info))
        );
        assert_eq!(
            crate::Header::parse(&p, (p.len() * 3) as u64)
                .unwrap()
                .read_profile(),
            Err(InspectError::Encoded)
        );
    }
}

#[test]
fn jet_encoding_is_separate_from_unknown_password_status() {
    for code in [1, 2, 3, 5, 6] {
        let p = page(code, false);
        assert_eq!(
            inspect_encryption(&mut source(&p), &mut budget(), &|| false),
            Ok(EncryptionInspection::UnencodedPages)
        );
    }
    let p = page(1, true);
    assert_eq!(
        inspect_encryption(&mut source(&p), &mut budget(), &|| false),
        Ok(EncryptionInspection::JetEncoding)
    );
    for encoded in [false, true] {
        let p = page(0, encoded);
        assert_eq!(
            inspect_encryption(&mut source(&p), &mut budget(), &|| false),
            Err(InspectError::Unsupported)
        );
    }
    let mut p = page(5, true);
    p[21] = 2;
    assert_eq!(
        inspect_encryption(&mut source(&p), &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    );
}

#[test]
fn known_fixed_prefixes_and_unknown_versions_preserve_only_metadata() {
    let mut p = page(5, true);
    envelope(&mut p, 1, 1, &[0; 48]);
    assert_eq!(
        parse_office(&p).unwrap().family(),
        OfficeEncryptionFamily::BinaryRc4
    );
    for major in 2..=4 {
        envelope(&mut p, major, 2, &0x24_u32.to_le_bytes());
        let info = parse_office(&p).unwrap();
        assert_eq!(info.family(), OfficeEncryptionFamily::StandardOrCryptoApi);
        assert_eq!(info.flags(), Some(0x24));
    }
    for major in [3, 4] {
        envelope(&mut p, major, 3, &0x10_u32.to_le_bytes());
        assert_eq!(
            parse_office(&p).unwrap().family(),
            OfficeEncryptionFamily::Extensible
        );
    }
    envelope(&mut p, 99, 1, &[]);
    let info = parse_office(&p).unwrap();
    assert_eq!(info.family(), OfficeEncryptionFamily::Unknown);
    assert_eq!(info.version(), (99, 1));
}

#[test]
fn office_subversion_one_is_inspectable_without_changing_table_profile_gates() {
    for code in [2, 3, 5, 6] {
        let mut p = page(code, true);
        p[21] = OFFICE_ENVELOPE_SUBVERSION;
        envelope(&mut p, 4, 4, &[0x40, 0, 0, 0, b'<']);
        assert!(matches!(
            inspect_encryption(&mut source(&p), &mut budget(), &|| false),
            Ok(EncryptionInspection::Office(_))
        ));
        assert_eq!(
            crate::Header::parse(&p, (p.len() * 3) as u64)
                .unwrap()
                .read_profile(),
            Err(InspectError::Encoded)
        );
    }
    let mut jet = page(1, true);
    jet[21] = OFFICE_ENVELOPE_SUBVERSION;
    assert_eq!(
        inspect_encryption(&mut source(&jet), &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    );
}

#[test]
fn malformed_lengths_fixed_prefixes_and_reserved_word_fail_explicitly() {
    let mut p = page(5, true);
    for length in [0_u16, 1, 3, u16::MAX] {
        p[OFFICE_INFO_LENGTH_OFFSET..OFFICE_INFO_OFFSET].copy_from_slice(&length.to_le_bytes());
        assert_eq!(parse_office(&p), Err(InspectError::MalformedEncryption));
    }
    for (major, minor, tail) in [
        (1, 1, &[][..]),
        (4, 2, &[0, 0, 0][..]),
        (3, 3, &[][..]),
        (4, 4, &[0x40, 0, 0, 0][..]),
        (4, 4, &[0x41, 0, 0, 0, b'<'][..]),
    ] {
        envelope(&mut p, major, minor, tail);
        assert_eq!(parse_office(&p), Err(InspectError::MalformedEncryption));
    }
    assert_eq!(
        parse_office(&p[..OFFICE_INFO_OFFSET - 1]),
        Err(InspectError::Truncated)
    );
}

#[test]
fn cumulative_budget_cancellation_and_source_errors_remain_visible() {
    let mut p = page(5, true);
    envelope(&mut p, 4, 4, &[0x40, 0, 0, 0, b'<']);
    let mut input = source(&p);
    let original = input.get_ref().clone();
    assert!(matches!(
        inspect_encryption(
            &mut input,
            &mut ResourceBudget::new(p.len() as u64, 0, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(input.get_ref(), &original);
    let exact_bytes = crate::Header::PREFIX_BYTES as u64 + p.len() as u64;
    let mut exact = ResourceBudget::new(exact_bytes, 0, 100);
    assert!(matches!(
        inspect_encryption(&mut input, &mut exact, &|| false),
        Ok(EncryptionInspection::Office(_))
    ));
    assert_eq!(exact.remaining(Resource::Bytes), 0);
    let mut short = ResourceBudget::new(exact_bytes - 1, 0, 100);
    assert!(matches!(
        inspect_encryption(&mut input, &mut short, &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(
        inspect_encryption(&mut input, &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    );
    let calls = Cell::new(0);
    assert_eq!(
        inspect_encryption(&mut input, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() > 3
        }),
        Err(InspectError::Cancelled)
    );
    assert_eq!(input.get_ref(), &original);
    assert_eq!(
        inspect_encryption(&mut Cursor::new(vec![0; 10]), &mut budget(), &|| false),
        Err(InspectError::Truncated)
    );
}

proptest! {
    #[test]
    fn arbitrary_bounded_inputs_never_panic(data in prop::collection::vec(any::<u8>(), 0..=8192)) {
        let _ = parse_office(&data);
    }
}

use crate::{EncryptedSource, InspectError, NativeImage, NativeReadSource, inspect_header};
use std::io::{Cursor, Read, Seek, SeekFrom};
use tablecraft_core::limits::ResourceBudget;

// Original JCE/BC results from docs/testing/LegacyEncryptionVectors.java.
const VECTORS: &str = include_str!("vectors.txt");

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 100_000)
}
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn word(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn image(vector: &str, code: u8) -> Vec<u8> {
    let parts: Vec<_> = vector.split_whitespace().collect();
    let bits: u32 = parts[1].parse().unwrap();
    let mut page = vec![0; 3 * 4096];
    page[..4].copy_from_slice(&[0, 1, 0, 0]);
    page[4..20].copy_from_slice(if parts[0] == "Jet" {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    });
    page[20] = code;
    page[62..66].copy_from_slice(&[0xae, 0xec, 0xcb, 0xc6]); // masked 55 66 77 88
    if parts[0] != "Jet" {
        let mut descriptor = Vec::new();
        let hash = hex(parts[3]);
        if parts[0] == "Binary" {
            descriptor.extend_from_slice(&[1, 0, 1, 0]);
        } else {
            descriptor.extend_from_slice(&[4, 0, 2, 0]);
            let flags = if parts[0] == "Standard" { 0x24 } else { 4 };
            word(&mut descriptor, flags);
            word(&mut descriptor, 34);
            word(&mut descriptor, flags);
            word(&mut descriptor, 0);
            word(
                &mut descriptor,
                if parts[0] == "CryptoAPI" {
                    0x6801
                } else {
                    match bits {
                        128 => 0x660e,
                        192 => 0x660f,
                        _ => 0x6610,
                    }
                },
            );
            word(&mut descriptor, 0x8004);
            word(&mut descriptor, bits);
            word(
                &mut descriptor,
                if parts[0] == "CryptoAPI" { 1 } else { 24 },
            );
            word(&mut descriptor, 0);
            word(&mut descriptor, 0);
            descriptor.extend_from_slice(&[0, 0]);
            word(&mut descriptor, 16);
        }
        descriptor.extend_from_slice(&hex("000102030405060708090a0b0c0d0e0f"));
        descriptor.extend_from_slice(&hex(parts[2]));
        if parts[0] != "Binary" {
            word(&mut descriptor, 20);
        }
        descriptor.extend_from_slice(&hash);
        page[0x299..0x29b].copy_from_slice(&(descriptor.len() as u16).to_le_bytes());
        page[0x29b..0x29b + descriptor.len()].copy_from_slice(&descriptor);
    }
    page[4096..4096 + 32].copy_from_slice(&hex(parts[4]));
    page
}

#[test]
fn independent_vectors_cover_all_qualified_families_and_legacy_algorithms() {
    for vector in VECTORS.lines() {
        let jet = vector.starts_with("Jet ");
        for &code in if jet { &[1][..] } else { &[2, 3, 5, 6][..] } {
            let bytes = image(vector, code);
            let mut source = EncryptedSource::open(
                Cursor::new(bytes.clone()),
                if jet { None } else { Some("A😀é") },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let header = inspect_header(&mut source, &mut budget(), &|| false).unwrap();
            assert!(
                source
                    .qualify_header(header)
                    .unwrap()
                    .read_profile()
                    .is_ok()
            );
            let mut raw = [0; 32];
            source.seek(SeekFrom::Start(4096)).unwrap();
            source.read_exact(&mut raw).unwrap();
            assert_eq!(raw, bytes[4096..4096 + 32]);
            let mut decoded = bytes[4096..8192].to_vec();
            source
                .decode_native_page(1, &mut decoded, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(&decoded[..32], &[0; 32], "code {code}: {vector}");
            assert!(matches!(
                NativeImage::capture(
                    &mut source,
                    tablecraft_model::CatalogScope::new(1),
                    &mut budget(),
                    &|| false
                ),
                Err(InspectError::Encoded)
            ));
            assert_eq!(source.into_inner().into_inner(), bytes);
        }
    }
}

#[test]
fn credentials_lengths_and_cancellation_fail_without_modifying_ciphertext() {
    for vector in VECTORS.lines().filter(|v| !v.starts_with("Jet ")) {
        let bytes = image(vector, 5);
        for (password, expected) in [
            (None, InspectError::PasswordRequired),
            (Some("wrong"), InspectError::IncorrectPassword),
            (Some(""), InspectError::IncorrectPassword),
        ] {
            assert!(
                matches!(EncryptedSource::open(Cursor::new(&bytes), password, &mut budget(), &|| false), Err(error) if error == expected)
            );
        }
        let long = "x".repeat(256);
        assert!(matches!(
            EncryptedSource::open(Cursor::new(&bytes), Some(&long), &mut budget(), &|| false),
            Err(InspectError::InvalidPasswordLength)
        ));
        let mut source =
            EncryptedSource::open(Cursor::new(&bytes), Some("A😀é"), &mut budget(), &|| {
                false
            })
            .unwrap();
        let original = bytes[4096..8192].to_vec();
        let mut page = original.clone();
        assert_eq!(
            source.decode_native_page(1, &mut page, &mut budget(), &|| true),
            Err(InspectError::Cancelled)
        );
        assert_eq!(page, original);
        assert!(
            source
                .decode_native_page(1, &mut page, &mut ResourceBudget::new(0, 0, 1_000), &|| {
                    false
                })
                .is_err()
        );
        assert_eq!(page, original);
        let calls = std::cell::Cell::new(0);
        assert_eq!(
            source.decode_native_page(1, &mut page, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == 2
            }),
            Err(InspectError::Cancelled)
        );
        assert_eq!(page, original);
        // Exhaust work at the final publication check, after decrypting scratch.
        assert!(
            source
                .decode_native_page(
                    1,
                    &mut page,
                    &mut ResourceBudget::new(4096, 0, 257),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(page, original);
        for number in [0, 3, u32::MAX] {
            assert_eq!(
                source.decode_native_page(number, &mut page, &mut budget(), &|| false),
                Err(InspectError::MalformedEncryption)
            );
            assert_eq!(page, original);
        }
    }
}

#[test]
fn descriptor_truncation_and_unqualified_parameters_are_refused_before_credentials() {
    let vector = VECTORS
        .lines()
        .find(|v| v.starts_with("Standard 256 "))
        .unwrap();
    let original = image(vector, 5);
    let length = u16::from_le_bytes(original[0x299..0x29b].try_into().unwrap());
    for length in 0..length {
        let mut bytes = original.clone();
        bytes[0x299..0x29b].copy_from_slice(&length.to_le_bytes());
        let error = EncryptedSource::open(Cursor::new(bytes), None, &mut budget(), &|| false);
        assert!(matches!(
            error,
            Err(InspectError::MalformedEncryption | InspectError::UnsupportedEncryption)
        ));
    }
    for (offset, value) in [
        (0x29b + 4, 0x10u32),
        (0x29b + 12, 0),
        (0x29b + 24, 0x800c),
        (0x29b + 28, 257),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            EncryptedSource::open(Cursor::new(bytes), None, &mut budget(), &|| false),
            Err(InspectError::UnsupportedEncryption | InspectError::MalformedEncryption)
        ));
    }
}

#[test]
fn legacy_password_changes_and_removal_preserve_all_native_pages_across_profiles() {
    let plain = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let allowance = || ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000);
    for vector in VECTORS.lines().filter(|v| !v.starts_with("Jet ")) {
        for code in [2, 3, 5, 6] {
            let mut encrypted = plain.clone();
            encrypted[20] = code;
            let envelope = image(vector, code);
            encrypted[62..66].copy_from_slice(&envelope[62..66]);
            encrypted[0x299..0xe00].copy_from_slice(&envelope[0x299..0xe00]);
            let mut source = Cursor::new(&encrypted);
            let decoder = super::read(&mut source, &mut allowance(), &|| false)
                .unwrap()
                .unlock(Some("A😀é"), &mut allowance(), &|| false)
                .unwrap();
            for (number, page) in encrypted
                .as_chunks_mut::<4096>()
                .0
                .iter_mut()
                .enumerate()
                .skip(1)
            {
                decoder
                    .encode_page(number as u32, page, &mut allowance(), &|| false)
                    .unwrap();
            }
            let mut source = Cursor::new(&encrypted);
            let removed = crate::rewrite_encryption(
                &mut source,
                crate::EncryptionRewrite::Remove {
                    password: Some("A😀é"),
                },
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            assert_eq!(&removed.as_bytes()[4096..], &plain[4096..]);
            let mut paged = Cursor::new(Vec::new());
            crate::rewrite_encryption_to(
                &mut source,
                &mut paged,
                crate::EncryptionRewrite::Remove {
                    password: Some("A😀é"),
                },
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            assert!(paged.get_ref() == removed.as_bytes());

            let mut changed_pages = Cursor::new(Vec::new());
            crate::rewrite_encryption_to(
                &mut source,
                &mut changed_pages,
                crate::EncryptionRewrite::ChangePassword {
                    password: Some("A😀é"),
                    new_password: "new 😀 public password",
                },
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            let recovered_pages = crate::rewrite_encryption(
                &mut changed_pages,
                crate::EncryptionRewrite::Remove {
                    password: Some("new 😀 public password"),
                },
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            assert!(recovered_pages.as_bytes() == removed.as_bytes());
            assert!(
                !crate::inspect_header(
                    &mut Cursor::new(removed.as_bytes()),
                    &mut allowance(),
                    &|| false
                )
                .unwrap()
                .encoded_pages()
            );
            let changed = crate::rewrite_encryption(
                &mut source,
                crate::EncryptionRewrite::ChangePassword {
                    password: Some("A😀é"),
                    new_password: "new 😀 public password",
                },
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            assert_eq!(&changed.as_bytes()[..152], &encrypted[..152]);
            assert!(matches!(
                EncryptedSource::open(
                    Cursor::new(changed.as_bytes()),
                    Some("A😀é"),
                    &mut allowance(),
                    &|| false
                ),
                Err(InspectError::IncorrectPassword)
            ));
            let mut reopened = EncryptedSource::open(
                Cursor::new(changed.as_bytes()),
                Some("new 😀 public password"),
                &mut allowance(),
                &|| false,
            )
            .unwrap();
            for (number, page) in changed
                .as_bytes()
                .as_chunks::<4096>()
                .0
                .iter()
                .enumerate()
                .skip(1)
            {
                let mut decoded = *page;
                reopened
                    .decode_native_page(number as u32, &mut decoded, &mut allowance(), &|| false)
                    .unwrap();
                assert_eq!(
                    decoded.as_slice(),
                    &plain[number * 4096..(number + 1) * 4096],
                    "code {code}: {vector}"
                );
            }
            assert_eq!(source.into_inner(), &encrypted);
        }
    }
}

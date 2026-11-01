//! Selected declaration tests only; no property admission or mutation runs.
use super::validate_user;
use crate::{NativeImage, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(128);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn source(header: u8) -> Vec<u8> {
    let directory = PathBuf::from(
        env::var_os("TABLECRAFT_SELECTED_LVAL_SOURCE_DIR")
            .expect("provide the unchanged fixed-scalar genuine source directory"),
    );
    assert!(directory.is_absolute());
    let bytes = fs::read(directory.join(format!("h{header}.accdb"))).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let (length, expected) = if header == 5 {
        (
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        )
    } else {
        assert_eq!(header, 6);
        (
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        )
    };
    assert_eq!(bytes.len(), length);
    assert_eq!(digest, expected);
    bytes
}

fn definition(header: u8) -> usize {
    (if header == 5 { 107 } else { 125 }) * PAGE
}

fn validate(bytes: &[u8]) -> Result<(), crate::EditError> {
    let image = NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )?;
    assert_eq!(schema.columns().len(), 3);
    let result = validate_user(&image, &schema, &mut budget(), &|| false);
    assert_eq!(image.as_bytes(), bytes);
    result
}

#[test]
#[ignore = "requires two explicitly supplied immutable genuine source originals"]
fn selected_genuine_ole_declarations_keep_their_exact_nonzero_lval_pair() {
    for header in [5, 6] {
        let bytes = source(header);
        let td = definition(header);
        let length = u32::from_le_bytes(bytes[td + 8..td + 12].try_into().unwrap()) as usize;
        let pair = td + length - 12;
        assert_eq!(&bytes[td + 63 + 2 * 25..td + 63 + 2 * 25 + 1], &[11]);
        assert_eq!(&bytes[pair..pair + 2], &[2, 0]);
        assert_ne!(&bytes[pair + 2..pair + 6], &[0; 4]);
        assert_ne!(&bytes[pair + 6..pair + 10], &[0; 4]);
        assert_eq!(&bytes[pair + 10..pair + 12], &[255, 255]);
        validate(&bytes).unwrap();
    }
}

#[test]
#[ignore = "requires two explicitly supplied immutable genuine source originals"]
fn selected_lval_tail_and_unknown_descriptor_derivatives_refuse() {
    for header in [5, 6] {
        let original = source(header);
        let td = definition(header);
        let length = u32::from_le_bytes(original[td + 8..td + 12].try_into().unwrap()) as usize;
        let pair = td + length - 12;
        let field = td + 63 + 2 * 25;
        for variant in 0..9 {
            let mut bytes = original.clone();
            match variant {
                0 => bytes[pair..pair + 2].copy_from_slice(&[0, 0]), // non-LVAL field
                1 => bytes[pair..pair + 2].copy_from_slice(&[255, 255]), // omitted pair
                2 => bytes[pair + 2..pair + 6].fill(0),
                3 => bytes[pair + 6..pair + 10].fill(0),
                4 => bytes[field + 16] = 8, // unknown complex extension
                5 => bytes[field + 16] = 0xc0, // calculated extension
                6 => bytes[field + 15] |= 0x80, // unsupported field flag
                7 => {
                    // Keep the TD's length/free arithmetic self-consistent while
                    // adding unclassified bytes after the actual terminator.
                    bytes[td + 8..td + 12].copy_from_slice(&((length + 2) as u32).to_le_bytes());
                    bytes[td + 2..td + 4]
                        .copy_from_slice(&((PAGE - length - 10) as u16).to_le_bytes());
                }
                8 => bytes[td + 63 + 25 + 14] = 2, // unsupported Text sort version
                _ => unreachable!(),
            }
            assert!(
                validate(&bytes).is_err(),
                "header {header}, variant {variant}"
            );
        }
    }
}

#[test]
#[ignore = "requires two explicitly supplied immutable genuine source originals"]
fn selected_memo_descriptor_law_is_separate_from_payload_qualification() {
    for header in [5, 6] {
        for compression in [0, 1] {
            let mut bytes = source(header);
            let at = definition(header) + 63 + 2 * 25;
            bytes[at] = 12; // Memo, keeping the original LVAL field number/pair
            bytes[at + 11..at + 15].copy_from_slice(&[9, 4, 0, 1]);
            bytes[at + 16] = compression;
            // Deliberately only raw declaration coverage. Retyping binary
            // payload bytes does not establish a healthy Memo row/source.
            validate(&bytes).unwrap();
        }
    }
}

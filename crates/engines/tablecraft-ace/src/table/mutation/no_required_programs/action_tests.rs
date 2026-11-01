//! The action declaration mode retains the original source and program guards.
use super::NoRequiredTablePrograms;
use crate::{EditError, NativeImage, RowInsert, TableSchema, WriteBlocker, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(218);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn schema(image: &NativeImage) -> TableSchema {
    inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap()
}

fn originals() -> Vec<(Vec<u8>, usize, usize)> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GENUINE_NO_REQUIRED_SOURCES_DIR").unwrap());
    assert!(directory.is_absolute());
    [
        (
            "h5-distinct.accdb",
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            503,
            576,
        ),
        (
            "h6-distinct.accdb",
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            213,
            286,
        ),
    ]
    .into_iter()
    .map(|(name, expected, start, end)| {
        let bytes = fs::read(directory.join(name)).unwrap();
        let hash = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
        (bytes, 17 * 4096 + start, 17 * 4096 + end)
    })
    .collect()
}

#[test]
#[ignore = "requires the two immutable genuine no-required-program originals"]
fn action_certificate_binds_the_same_image_even_when_another_image_has_identical_bytes() {
    for (bytes, _, _) in originals() {
        let original = image(&bytes);
        let target = schema(&original);
        let certificate =
            NoRequiredTablePrograms::capture_actions(&original, &target, &mut budget(), &|| false)
                .unwrap();
        certificate
            .require_source(&original, &target, &mut budget(), &|| false)
            .unwrap();
        let foreign = image(&bytes);
        let foreign_schema = schema(&foreign);
        assert_eq!(original.as_bytes(), foreign.as_bytes());
        assert_eq!(
            certificate.require_source(&foreign, &foreign_schema, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        assert_eq!(original.as_bytes(), bytes);
        assert_eq!(foreign.as_bytes(), bytes);
    }
}

#[test]
#[ignore = "requires immutable genuine catalog geometry before action producers"]
fn action_certificate_and_empty_or_nonempty_insert_refuse_a_null_mask_hiding_unknown_extra_bytes() {
    for (bytes, start, end) in originals() {
        assert_eq!(&bytes[end - 3..end], &[0xff, 0, 0]);
        assert_eq!(&bytes[end - 5..end - 3], &[11, 0]);
        let count_at = end - 5;
        for offset in 0..=11 {
            let at = count_at - 2 * (offset + 1);
            let expected = match offset {
                0 => 32,
                1 => 42,
                _ => 44,
            };
            assert_eq!(
                u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()),
                expected
            );
        }
        let mut changed = bytes.clone();
        // Owner's variable end becomes43. The following eight NULL remote/code
        // spans remain empty; only final LvExtra spans43..44. All native origin,
        // name, fixed fields, row extent and NULL bits stay exact. Owner is an
        // opaque binary carrier and its one-byte extent remains well framed.
        for offset in 2..=10 {
            let at = count_at - 2 * (offset + 1);
            changed[at..at + 2].copy_from_slice(&43_u16.to_le_bytes());
        }
        assert_eq!(&changed[end - 3..end], &[0xff, 0, 0]);
        assert_eq!(&changed[start..start + 44], &bytes[start..start + 44]);
        assert_eq!(
            u16::from_le_bytes(changed[count_at - 22..count_at - 20].try_into().unwrap()),
            43
        );
        assert_eq!(
            u16::from_le_bytes(changed[count_at - 24..count_at - 22].try_into().unwrap()),
            44
        );
        let source = image(&changed);
        let target = schema(&source);
        assert_eq!(
            NoRequiredTablePrograms::capture_actions(&source, &target, &mut budget(), &|| false)
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::ActiveContent)
        );
        for empty in [false, true] {
            let rows = if empty {
                vec![]
            } else {
                vec![RowInsert::new(vec![
                    Value::Int32(4),
                    Value::Int32(17),
                    Value::Int32(3),
                ])]
            };
            let error = image(&changed)
                .plan_ordinary_complete_row_inserts("Items", &rows, &mut budget(), &|| false)
                .err()
                .unwrap();
            assert_eq!(error, EditError::Unsupported(WriteBlocker::ActiveContent));
        }
        assert_eq!(source.as_bytes(), changed);
    }
}

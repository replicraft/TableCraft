//! Metadata-only catalog gaps never become ordinary row-write permission.
use super::*;
use crate::{inspect_table_schema, table::mutation::preserved_source};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

const SCOPE: CatalogScope = CatalogScope::new(995);
const PAGE: usize = 4096;
const CATALOG: usize = 4;
const USER_SLOT: usize = 7;
const DIRECTORY: usize = 14;
const MASK_BYTES: usize = 3;
const BODY_NUMBER: usize = 13;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn catalog_gap(before: &[u8]) -> (usize, usize, usize, usize) {
    let page = &before[CATALOG * PAGE..(CATALOG + 1) * PAGE];
    let root = word(page, DIRECTORY + USER_SLOT * 2);
    let slot = if root & 0x4000 != 0 {
        let at = root & 0x1fff;
        let reference = u32::from_le_bytes(page[at..at + 4].try_into().unwrap());
        assert_eq!(reference >> 8, CATALOG as u32);
        reference as usize & 255
    } else {
        USER_SLOT
    };
    let start = word(page, DIRECTORY + slot * 2) & 0x1fff;
    let end = word(page, DIRECTORY + (slot - 1) * 2) & 0x1fff;
    let row = &page[start..end];
    let count_at = row.len() - MASK_BYTES - 2;
    let variables = word(row, count_at);
    assert_eq!(variables, 11);
    let footer = count_at - 2 * (variables + 1);
    let variable_end = word(row, footer);
    assert_eq!(footer - variable_end, 95);
    // The independent inline-to-external fixture retains the old record
    // capacity but initializes its unreferenced gap to zero. The positive test
    // below changes an opaque byte deliberately and requires preservation.
    assert_eq!(&row[variable_end..footer], &[0; 95]);
    (CATALOG * PAGE + start, end - start, variable_end, footer)
}

#[test]
fn exact_existing_inline_backing_gap_is_schema_only_and_stays_byte_identical() {
    let mut before = fitting::source(SourceCase::InlineNumberThenText);
    let (row_start, _, gap_start, footer) = catalog_gap(&before);
    before[row_start + gap_start + 6] ^= 0xff;
    let original = image(&before);
    let schema = inspect_table_schema(
        &mut original.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        NoRequiredTablePrograms::capture_updates(&original, &schema, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::ActiveContent))
    ));

    let certificate =
        NoRequiredTablePrograms::capture_current_schema(&original, &schema, &mut budget(), &|| {
            false
        })
        .unwrap();
    certificate
        .require_current_schema_source(&original, &schema, &mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        certificate.require_source(&original, &schema, &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        certificate.retained_update_keys(),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        certificate.require_generated_insert(),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        preserved_source::capture_no_required_actions(
            &original,
            "Table1",
            &certificate,
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        certificate
            .require_current_schema_source(&image(&before), &schema, &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));

    let expected = fitting::rename_current(&before, "Label", "Title");
    let candidate = original
        .prepare_current_field_rename("Table1", "Label", "Title", &mut budget(), &|| false)
        .unwrap();
    assert!(candidate.after_bytes() == expected);
    assert_eq!(
        &candidate.after_bytes()[row_start + gap_start..row_start + footer],
        &before[row_start + gap_start..row_start + footer]
    );
}

#[test]
fn retained_gap_cannot_overlap_footer_or_activate_a_code_carrier() {
    let before = fitting::source(SourceCase::InlineNumberThenText);
    let (row_start, row_len, _, footer) = catalog_gap(&before);
    let mut malformed_offset = before.clone();
    malformed_offset[row_start + footer..row_start + footer + 2]
        .copy_from_slice(&((footer + 1) as u16).to_le_bytes());
    let mut active_body = before;
    active_body[row_start + row_len - MASK_BYTES + BODY_NUMBER / 8] |= 1 << (BODY_NUMBER % 8);

    for bytes in [malformed_offset, active_body] {
        let admit = || -> Result<(), EditError> {
            let original =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)?;
            let schema = inspect_table_schema(
                &mut original.reader(),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )?;
            NoRequiredTablePrograms::capture_current_schema(
                &original,
                &schema,
                &mut budget(),
                &|| false,
            )
            .map(drop)
        };
        assert!(admit().is_err());
    }
}

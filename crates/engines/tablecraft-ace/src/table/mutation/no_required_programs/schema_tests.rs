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
    let start = word(page, DIRECTORY + USER_SLOT * 2);
    assert!(start < PAGE);
    let end = word(page, DIRECTORY + (USER_SLOT - 1) * 2);
    let row = &page[start..end];
    let count_at = row.len() - MASK_BYTES - 2;
    let variables = word(row, count_at);
    assert_eq!(variables, 11);
    let footer = count_at - 2 * (variables + 1);
    let variable_end = word(row, footer);
    assert_eq!(footer - variable_end, 95);
    assert_eq!(&row[variable_end..variable_end + 4], b"MR2\0");
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
        NoRequiredTablePrograms::capture_schema(&original, &schema, &mut budget(), &|| false)
            .unwrap();
    certificate
        .require_schema_source(&original, &schema, &mut budget(), &|| false)
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
        certificate.require_schema_source(&image(&before), &schema, &mut budget(), &|| false),
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
        let original = image(&bytes);
        let schema = inspect_table_schema(
            &mut original.reader(),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            NoRequiredTablePrograms::capture_schema(&original, &schema, &mut budget(), &|| false)
                .is_err()
        );
    }
}

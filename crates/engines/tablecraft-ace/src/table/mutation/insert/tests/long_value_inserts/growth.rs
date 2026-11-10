//! Existing column references survive owned growth and empty-candidate rebasing.
pub(in crate::table::mutation::insert) mod model;
use super::{PAGE, original};
use crate::{
    EditError, LongValueCheckStatus, RowInsert, WriteBlocker, check_table_long_values, read_table,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn literal_growth_sources_and_expected_images_qualify_before_preview() {
    for code in [5, 6] {
        for (eof, length, empty) in [
            (527, 65, false),
            (528, 65, false),
            (528, 8145, false),
            (600, 65, true),
        ] {
            let case = model::source_and_expected(code, eof, length, empty);
            model::checked(&case.before, &model::rows(None));
            model::checked(&case.expected, &model::rows(Some(&case.value)));
            assert_eq!(
                case.expected.len(),
                case.before.len() + (case.bodies.len() + case.bitmaps.len()) * PAGE
            );
            assert_eq!(case.empty, empty);
        }
    }
}

#[test]
fn owned_rebase_promotion_reused_bitmap_and_empty_maps_match_complete_literal_images() {
    for code in [5, 6] {
        for (eof, length, empty) in [
            (527, 65, false),
            (528, 65, false),
            (528, 8145, false),
            (600, 65, true),
        ] {
            let case = model::source_and_expected(code, eof, length, empty);
            model::checked(&case.before, &model::rows(None));
            model::checked(&case.expected, &model::rows(Some(&case.value)));
            let requests = [RowInsert::new(vec![
                Value::Int32(10),
                Value::Int32(100),
                Value::Binary(case.value.clone()),
            ])];
            let image = original::capture(&case.before);
            let candidate = image
                .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
                .unwrap();
            let actual = candidate.candidate_bytes();
            assert_eq!(actual.len(), case.expected.len());
            let difference = actual
                .iter()
                .zip(&case.expected)
                .position(|(actual, expected)| actual != expected);
            assert!(
                difference.is_none(),
                "code{code}, EOF{eof}, length{length}, empty{empty}: first difference{difference:?}"
            );
            model::checked(actual, &model::rows(Some(&case.value)));
        }
    }
}

#[test]
fn missing_or_zero_column_maps_refuse_without_initialization_or_source_changes() {
    for missing in [false, true] {
        let mut case = model::source_and_expected(6, 528, 65, false);
        if missing {
            case.before[10 * PAGE + 498..10 * PAGE + 500].copy_from_slice(&65535_u16.to_le_bytes());
        } else {
            case.before[10 * PAGE + 500..10 * PAGE + 504].fill(0);
        }
        super::checked(&case.before, &model::rows(None));
        let image = original::capture(&case.before);
        let result = image.preview_ace16_insert_rows(
            "Items",
            &[RowInsert::new(vec![
                Value::Int32(10),
                Value::Int32(100),
                Value::Binary(vec![0x92; 65]),
            ])],
            &mut original::budget(),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}

#[test]
fn full_insert_above_two_mebibytes_and_1024_chunks_matches_independent_image() {
    let case = model::source_and_expected(6, 528, 4_169_729, false);
    model::checked(&case.before, &model::rows(None));
    assert_eq!(case.bodies.len(), 1025);
    assert_eq!(case.bitmaps, [529]);
    assert_eq!(case.expected.len(), 1554 * PAGE);
    // The existing explicit per-operation test budget is unchanged. This case
    // requests one row, but traverses and independently reconstructs 1025 bodies.
    // NO_CLONE: The original caller request owns its payload while the immutable
    // independent case retains the expected bytes for the final readback check.
    let requests = [RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        Value::Binary(case.value.clone()),
    ])];
    let candidate = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
        .unwrap();
    let actual = candidate.candidate_bytes();
    assert_eq!(actual.len(), case.expected.len());
    assert!(
        actual == case.expected,
        "first differing byte {:?}",
        actual.iter().zip(&case.expected).position(|(a, b)| a != b)
    );
    original::checked(actual, 4, 4);
    let snapshot = read_table(
        &mut Cursor::new(actual),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(&snapshot.rows()[..3], model::rows(None));
    let inserted = &snapshot.rows()[3];
    assert_eq!(inserted[..2], [Value::Int32(10), Value::Int32(100)]);
    let Value::Binary(payload) = &inserted[2] else {
        panic!("the new OLE column must read back as binary");
    };
    assert_eq!(payload.len(), 4_169_729);
    assert!(
        payload == &case.value,
        "new OLE payload differs from the original request"
    );
    let report = check_table_long_values(
        &mut Cursor::new(actual),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let column = &report.columns()[0];
    assert_eq!(column.status(), LongValueCheckStatus::Checked);
    assert_eq!(column.distinct_chunks(), 1025);
    assert_eq!(column.values().chained(), 1);
    assert_eq!(column.shared_references(), 0);
    assert_eq!(column.shared_owned_pages(), 0);
}

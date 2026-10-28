//! Existing column references survive owned growth and empty-candidate rebasing.
pub(in crate::table::mutation::insert) mod model;
use super::{PAGE, original};
use crate::{EditError, RowInsert, WriteBlocker};
use tablecraft_core::values::Value;

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

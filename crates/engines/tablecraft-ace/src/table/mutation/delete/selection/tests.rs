use super::*;
use crate::table::mutation::insert::tests::multiple_indexes::model;
use std::{cell::Cell, io::Cursor};
use tablecraft_model::CatalogScope;

const LIMITS: [u64; 3] = [256 * 1024 * 1024, 100_000, 100_000_000];

fn budget() -> ResourceBudget {
    ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2])
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn expected_records() -> [Vec<u8>; 3] {
    // Literal stored framing, including the variable footer and NULL mask.
    [
        vec![3, 0, 1, 0, 0, 0, 10, 0, 0, 0, 10, 0, 10, 0, 1, 0, 3],
        vec![3, 0, 2, 0, 0, 0, 10, 0, 0, 0, 10, 0, 10, 0, 1, 0, 7],
        vec![
            3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 128, 0, 255, 13, 0, 10, 0, 1, 0, 5,
        ],
    ]
}

#[test]
fn qualify_ordinal_delete_sources_and_raw_payloads_before_selection() {
    for code in [5, 6] {
        let case = model::source(code, false, false);
        let checked = crate::check_table(
            &mut Cursor::new(&case.before),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.schema().declared_rows(), 3);
        let report = crate::check_table_constraints(
            &mut Cursor::new(&case.before),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.rows(), 3);
        assert!(report.relationships().is_empty());
        assert_ne!(
            report.table_validation(),
            crate::RuleCheckStatus::Unsupported
        );
        for field in report.fields() {
            for status in [field.required(), field.empty_text(), field.validation()] {
                assert_ne!(status, crate::RuleCheckStatus::Unsupported);
            }
        }
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| { index.status() == crate::IndexCheckStatus::Checked })
        );
        for ((_, reference), expected) in case.rows.iter().zip(expected_records()) {
            let page = (*reference >> 8) as usize;
            let slot = (*reference & 255) as usize;
            let body = &case.before[page * 4096..(page + 1) * 4096];
            let word = |at: usize| usize::from(u16::from_le_bytes([body[at], body[at + 1]]));
            let start = word(14 + slot * 2) & 8191;
            let end = if slot == 0 {
                4096
            } else {
                word(12 + slot * 2) & 8191
            };
            assert_eq!(&body[start..end], expected);
        }
    }
}

#[test]
fn ordinals_capture_exact_null_empty_and_binary_records_in_source_order() {
    let expected = expected_records();
    for code in [5, 6] {
        let case = model::source(code, false, false);
        let image = capture(&case.before);
        let requests = image
            .prepare_ace16_row_deletes("Items", &[2, 0, 1], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(requests.len(), 3);
        for (ordinal, request) in requests.iter().enumerate() {
            assert_eq!(request.row(), ordinal);
            assert_eq!(request.expected_bytes(), expected[ordinal]);
        }
        assert_eq!(image.as_bytes(), case.before);
        let candidate = image
            .preview_ace16_deletes("Items", &requests, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.deleted_rows(), 3);
        assert_eq!(candidate.before_bytes(), case.before);
    }
}

#[test]
fn duplicates_missing_ordinals_and_stale_payloads_refuse_without_partial_requests() {
    let case = model::source(6, false, false);
    let image = capture(&case.before);
    for ordinals in [&[0, 0][..], &[0, 3], &[usize::MAX]] {
        assert!(matches!(
            image.prepare_ace16_row_deletes("Items", ordinals, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
    }
    let mut requests = image
        .prepare_ace16_row_deletes("Items", &[0], &mut budget(), &|| false)
        .unwrap();
    requests[0].expected[2] ^= 1;
    assert!(matches!(
        image.preview_ace16_deletes("Items", &requests, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
}

#[test]
fn empty_selection_keeps_admission_and_leaves_the_captured_source_unchanged() {
    let case = model::source(5, false, false);
    let image = capture(&case.before);
    assert!(
        image
            .prepare_ace16_row_deletes("Items", &[], &mut budget(), &|| false)
            .unwrap()
            .is_empty()
    );
    assert!(
        image
            .prepare_ace16_row_deletes("Missing", &[], &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(image.as_bytes(), case.before);
}

#[test]
fn selection_charges_each_pool_and_observes_terminal_cancellation() {
    let case = model::source(6, false, false);
    let image = capture(&case.before);
    let polls = Cell::new(0);
    let mut healthy = budget();
    image
        .prepare_ace16_row_deletes("Items", &[2, 0], &mut healthy, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        let used = LIMITS[position] - healthy.remaining(resource);
        assert!(used > 0);
        let mut limits = LIMITS;
        limits[position] = used - 1;
        let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
        assert!(matches!(
            image.prepare_ace16_row_deletes("Items", &[2, 0], &mut limited, &|| false),
            Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource
        ));
    }
    let final_poll = polls.get();
    polls.set(0);
    assert!(matches!(
        image.prepare_ace16_row_deletes("Items", &[2, 0], &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), final_poll);
    assert_eq!(image.as_bytes(), case.before);
}

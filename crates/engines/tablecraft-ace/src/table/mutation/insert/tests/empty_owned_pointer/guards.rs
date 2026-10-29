//! Higher source pointers and their metadata remain independent source authority.
use super::{maps, model, original};
use crate::{
    EditError, InspectError, RowInsert,
    table::mutation::insert::{audit, compose, preflight, prepare},
};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access as literal;

const PAGE: usize = 4096;

fn owner_start(bytes: &[u8]) -> usize {
    let start = u16::from_le_bytes(bytes[11 * PAGE + 14..11 * PAGE + 16].try_into().unwrap());
    11 * PAGE + usize::from(start)
}

fn with_last_pointer(code: u8) -> model::Case {
    let mut case = model::case(code);
    let start = owner_start(&case.before);
    // A 69-byte reference row has seventeen four-byte pointer slots. The last
    // source pointer remains meaningful even though its bitmap has no members.
    for bytes in [&mut case.before, &mut case.expected] {
        literal::put32(bytes, start + 65, 15);
        bytes[15 * PAGE..16 * PAGE].fill(0);
        bytes[15 * PAGE..15 * PAGE + 4].copy_from_slice(&[5, 1, 0, 0]);
    }
    case
}

fn qualified(case: &model::Case) {
    original::checked(&case.before, 0, 2);
    original::checked(&case.expected, 1, 2);
    maps(&case.before, &[], &[]);
    maps(&case.expected, &[16], &[16]);
    let start = owner_start(&case.before);
    assert_eq!(&case.before[start..start + 5], &[1, 0, 0, 0, 0]);
    assert_eq!(&case.before[start + 65..start + 69], &[15, 0, 0, 0]);
    assert_eq!(&case.expected[start + 65..start + 69], &[15, 0, 0, 0]);
    assert_eq!(
        &case.before[15 * PAGE..16 * PAGE],
        &case.expected[15 * PAGE..16 * PAGE]
    );
    assert_eq!(
        &case.before[12 * PAGE..13 * PAGE],
        &case.expected[12 * PAGE..13 * PAGE]
    );
    assert_eq!(case.expected[17 * PAGE + 6], 1);
}

#[test]
fn last_owned_pointer_sources_and_expected_images_qualify_before_preview() {
    for code in [5, 6] {
        qualified(&with_last_pointer(code));
    }
}

#[test]
fn initialized_block_zero_preserves_last_pointer_and_rejects_correlated_erasure() {
    for code in [5, 6] {
        let case = with_last_pointer(code);
        qualified(&case);

        let requests = [RowInsert::new(case.values.clone())];
        let image = original::capture(&case.before);
        let checked = preflight(&image, "Items", &requests, &mut original::budget(), &|| {
            false
        })
        .unwrap();
        let mut stage = prepare(
            &image,
            &checked,
            &requests,
            &mut original::budget(),
            &|| false,
        )
        .unwrap();

        let healthy = compose::compose(&image, &stage, &mut original::budget(), &|| false).unwrap();
        let difference = healthy
            .iter()
            .zip(&case.expected)
            .position(|(left, right)| left != right);
        assert_eq!(healthy.len(), case.expected.len());
        assert_eq!(difference, None);
        audit::prove(
            &image,
            "Items",
            &requests,
            &stage,
            &healthy,
            &mut original::budget(),
            &|| false,
        )
        .unwrap();

        let offset = owner_start(&case.before) + 65;
        let edit = stage
            .data
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == offset)
            .unwrap();
        assert_eq!((edit.before, edit.after), (15, 15));
        edit.after = 0;

        let forged = compose::compose(&image, &stage, &mut original::budget(), &|| false).unwrap();
        assert_ne!(forged, healthy);
        assert_eq!(
            audit::prove(
                &image,
                "Items",
                &requests,
                &stage,
                &forged,
                &mut original::budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn empty_owned_initialization_exhausts_all_pools_and_cancels_at_actual_final_callback() {
    let case = with_last_pointer(6);
    qualified(&case);
    let requests = [RowInsert::new(case.values)];
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(matches!(
            original::capture(&case.before).preview_ace16_insert_rows(
                "Items",
                &requests,
                &mut budget,
                &|| false
            ),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
    }

    let polls = Cell::new(0);
    let healthy = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let actual = healthy.candidate_bytes();
    let difference = actual
        .iter()
        .zip(&case.expected)
        .position(|(actual, expected)| actual != expected);
    assert_eq!(actual.len(), case.expected.len());
    assert_eq!(difference, None);

    let terminal = polls.get();
    assert!(terminal > 1);
    polls.set(0);

    assert!(matches!(
        original::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &requests,
            &mut original::budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            }
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(polls.get(), terminal);
}

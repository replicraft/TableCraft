//! Complete source/expected reads precede global-map promotion execution.
pub(in crate::table::mutation) mod model;

use super::model as original;
use crate::{
    EditError, InspectError, LongValueCheckStatus, RowInsert, check_table_long_values, read_table,
};
use model::{Case, Phase};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

pub(in crate::table::mutation::insert) fn checked(
    bytes: &[u8],
    rows: &[Vec<Value>],
    aliases: usize,
) {
    original::checked(bytes, rows.len(), aliases);
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows().len(), rows.len());
    for (position, (actual, expected)) in snapshot.rows().iter().zip(rows).enumerate() {
        assert_eq!(actual, expected, "stored row {position}");
    }
}

fn qualify(case: &Case, phase: Phase, hole: bool) {
    checked(&case.before, &case.old_rows, case.aliases);
    checked(&case.expected, &case.final_rows, case.aliases);
    if matches!(
        phase,
        Phase::LongValue | Phase::Container | Phase::OwnedBitmap
    ) {
        let report = check_table_long_values(
            &mut Cursor::new(&case.expected),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.columns()[0].status(), LongValueCheckStatus::Checked);
        assert_eq!(report.columns()[0].shared_owned_pages(), 0);
        assert_eq!(report.columns()[0].shared_references(), 0);
    }
    if let Some(bitmap) = case.global_bitmap {
        assert_eq!(bitmap, 513);
        let bytes = &case.expected[bitmap * PAGE..(bitmap + 1) * PAGE];
        assert_eq!(&bytes[..4], &[5, 1, 0, 0]);
        assert_eq!(&bytes[4..68], &[0; 64]);
        let used = case.expected.len() / PAGE - 512;
        let frontier = (255_u16 << used) as u8;
        assert_eq!(bytes[68], frontier);
        assert!(bytes[69..4092].iter().all(|byte| *byte == 255));
        assert_eq!(&bytes[4092..], &[0; 4]);
        assert_eq!(bytes[4 + 30 / 8] & (1 << (30 % 8)), 0);
    } else {
        assert!(matches!(phase, Phase::LongValue));
        assert_eq!(case.expected.len(), 512 * PAGE);
        // The in-file free hole survives when no promotion occurs.
        let start = PAGE + 4027;
        assert_eq!(
            case.expected[start + 5 + 30 / 8] & (1 << (30 % 8)) != 0,
            hole
        );
    }
}

#[test]
fn literal_global_sources_and_expected_images_qualify_for_all_allocation_phases() {
    for code in [5, 6] {
        for phase in [
            Phase::Data,
            Phase::Index,
            Phase::LongValue,
            Phase::Container,
            Phase::OwnedBitmap,
        ] {
            for eof in [511, 512] {
                for hole in [false, true] {
                    let case = model::case(code, phase, eof, hole);
                    qualify(&case, phase, hole);
                }
            }
        }
    }
}

#[test]
fn complete_global_promotion_images_match_for_each_phase_and_crossing_position() {
    for code in [5, 6] {
        for phase in [
            Phase::Data,
            Phase::Index,
            Phase::LongValue,
            Phase::Container,
            Phase::OwnedBitmap,
        ] {
            for eof in [511, 512] {
                for hole in [false, true] {
                    let case = model::case(code, phase, eof, hole);
                    qualify(&case, phase, hole);
                    let candidate = original::capture(&case.before)
                        .preview_ace16_insert_rows(
                            "Items",
                            &[RowInsert::new(case.values)],
                            &mut original::budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| {
                            panic!("code{code}, {phase:?}, EOF{eof}, hole{hole}: {error:?}")
                        });
                    let actual = candidate.candidate_bytes();

                    assert_eq!(actual.len(), case.expected.len());
                    let difference = actual.iter().zip(&case.expected).position(|(a, b)| a != b);
                    assert!(
                        difference.is_none(),
                        "code{code}, {phase:?}, EOF{eof}, hole{hole}: first difference {difference:?}"
                    );
                    checked(actual, &case.final_rows, case.aliases);
                }
            }
        }
    }
}

#[test]
fn global_and_owned_bitmap_coallocation_respects_each_pool_and_final_cancellation() {
    let case = model::case(6, Phase::OwnedBitmap, 512, true);
    qualify(&case, Phase::OwnedBitmap, true);
    let requests = [RowInsert::new(case.values)];
    let polls = Cell::new(0_usize);
    let mut healthy_budget = original::budget();
    let healthy = original::capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut healthy_budget, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    assert!(healthy.candidate_bytes() == case.expected);

    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let initial = original::budget().remaining(resource);
        let used = initial - healthy_budget.remaining(resource);
        assert!(used > 0);
        let mut limited = original::budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let result = original::capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &requests,
            &mut limited,
            &|| false,
        );

        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }

    let observed = Cell::new(0_usize);
    let result = original::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &requests,
        &mut original::budget(),
        &|| {
            observed.set(observed.get() + 1);
            observed.get() == polls.get()
        },
    );

    assert!(matches!(
        result,
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(observed.get(), polls.get());
}

#[test]
fn an_already_occupied_represented_eof_is_not_skipped_by_promoting_global_storage() {
    let mut case = model::case(6, Phase::LongValue, 511, false);
    case.before[PAGE + 4027 + 5 + 511 / 8] &= !(1 << (511 % 8));
    checked(&case.before, &case.old_rows, case.aliases);
    let result = original::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(case.values)],
        &mut original::budget(),
        &|| false,
    );

    assert!(matches!(result, Err(EditError::Preservation)));
}

#[test]
fn an_inline_global_origin_past_its_canonical_window_does_not_gain_growth_authority() {
    let mut case = model::case(6, Phase::LongValue, 512, false);
    case.before.resize(513 * PAGE, 0);
    checked(&case.before, &case.old_rows, case.aliases);
    let result = original::capture(&case.before).preview_ace16_insert_rows(
        "Items",
        &[RowInsert::new(case.values)],
        &mut original::budget(),
        &|| false,
    );

    assert!(result.is_err());
}

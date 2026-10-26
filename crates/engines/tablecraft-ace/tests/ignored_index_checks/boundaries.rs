use super::{SCOPE, budget, check, checked, corpus, numeric_fixture};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32Update, NativeImage,
    WriteBlocker, check_table, read_table_index,
};
use tablecraft_core::limits::Resource;

#[test]
fn missing_eligible_or_injected_all_null_rows_and_wrong_keys_refuse_the_whole_report() {
    let original = corpus::integer_pair(5, false, false);
    let retained = corpus::entries(&original, corpus::ROOT);
    let mut missing = original.clone();
    corpus::write_entries(&mut missing, corpus::ROOT, &retained[1..]);
    assert_eq!(check(&missing).err(), Some(InspectError::Malformed));

    let all_rows = [
        vec![Some(1), None, None],
        vec![Some(2), None, Some(0)],
        vec![Some(3), Some(0), None],
        vec![Some(4), Some(0), Some(0)],
        vec![Some(5), None, None],
        vec![Some(6), Some(1), None],
    ];
    let ordinary = numeric_fixture::database(5, &[4, 4, 4], &[(2, false), (1, true)], &all_rows, 0);
    let null_entry = corpus::entries(&ordinary, corpus::ROOT)
        .into_iter()
        .find(|entry| corpus::slot(entry) == 0)
        .unwrap();
    let mut entries = retained.clone();
    entries[0] = null_entry;
    entries.sort();
    let mut illegal_null = original.clone();
    corpus::write_entries(&mut illegal_null, corpus::ROOT, &entries);
    assert_eq!(check(&illegal_null).err(), Some(InspectError::Malformed));

    let mut duplicate = original.clone();
    let mut entries = retained;
    entries[1] = entries[0].clone();
    entries.sort();
    corpus::write_entries(&mut duplicate, corpus::ROOT, &entries);
    assert_eq!(check(&duplicate).err(), Some(InspectError::Malformed));

    let mut wrong_key = original.clone();
    corpus::wrong_second_component(&mut wrong_key, 1);
    assert_eq!(check(&wrong_key).err(), Some(InspectError::Malformed));
    checked(&original, 6);
}

#[test]
fn required_and_unique_checks_include_omitted_and_partially_null_roots() {
    let mut required = corpus::integer_pair(5, false, false);
    numeric_fixture::index_flags(&mut required, 0x8a);
    assert_eq!(
        check(&required).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey
        ))
    );

    let rows = [vec![Some(1), None, Some(7)], vec![Some(2), None, Some(7)]];
    let bytes = numeric_fixture::database(5, &[4, 4, 4], &[(1, false), (2, false)], &rows, 0);
    let duplicate = corpus::ignoring(bytes, &[], true);
    assert_eq!(
        check(&duplicate).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ))
    );
}

#[test]
fn public_full_reads_and_normal_or_preview_mutations_are_not_enabled_by_physical_checks() {
    for code in [1, 2, 5, 6] {
        let bytes = corpus::integer_pair(code, false, false);
        checked(&bytes, 6);
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "Compound",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Unsupported)
        );
        for updates in [Vec::new(), vec![Int32Update::new(0, 1, 99)]] {
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            match code {
                1 => assert_eq!(
                    image
                        .plan_int32_updates("Items", "Id", &updates, &mut budget(), &|| false)
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
                ),
                #[cfg(feature = "ace16-write-preview")]
                5 | 6 => assert_eq!(
                    image
                        .preview_ace16_int32_updates(
                            "Items",
                            "Id",
                            &updates,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn unqualified_flags_stay_visible_without_traversing_an_unqualified_root() {
    let mut bytes = corpus::integer_pair(5, false, false);
    numeric_fixture::index_flags(&mut bytes, 0x92);
    bytes[corpus::ROOT * corpus::PAGE_BYTES] = 0;
    let report = check(&bytes).unwrap();
    assert_eq!(report.rows(), 6);
    assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Unsupported);
}

#[test]
fn all_observed_cancellation_points_and_native_quota_failures_remain_atomic() {
    let bytes = corpus::integer_pair(5, true, false);
    let observed = Cell::new(0);
    check_table(
        &mut Cursor::new(&bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| {
            observed.set(observed.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        assert_eq!(
            check_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            )
            .err(),
            Some(InspectError::Cancelled)
        );
    }
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut allowance = budget();
        allowance
            .charge(resource, allowance.remaining(resource))
            .unwrap();
        assert!(matches!(
            check_table(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                &mut allowance,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    checked(&bytes, 6);
}

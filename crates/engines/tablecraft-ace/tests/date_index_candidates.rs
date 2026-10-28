#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{io::Cursor, ops::Bound};
use tablecraft_ace::{
    DateTimeIndexSelection, IndexProjectionSelection, IndexReadRequest, InspectError,
    read_table_index, read_table_index_in_table_order, read_table_index_with_request,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const INDEX_PAGE: usize = 14;
const DATA_PAGE: usize = 12;
const NULL_MASK_BYTES: usize = 1;
const PROFILE_OFFSET: usize = 20;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

fn date(serial: f64) -> LegacyDateTime {
    LegacyDateTime::from_ole_days(serial).unwrap()
}

fn samples() -> Vec<Option<u64>> {
    let mut values = vec![
        None,
        Some(0),
        Some(0x8000000000000000),
        Some(1),
        Some(0x8000000000000001),
    ];
    values.extend(
        [
            -657_434.75_f64,
            -2.75,
            -2.0,
            -1.75,
            -1.25,
            -1.0,
            -0.75,
            -0.25,
            0.25,
            0.75,
            1.0,
            1.25,
            45_000.125,
            2_958_465.999_999_999_5,
        ]
        .map(|value| Some(value.to_bits())),
    );
    values
}

fn database(profile: u8, descending: bool, values: &[Option<u64>]) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(position, bits)| vec![Some(position as i64 + 1), bits.map(|bits| bits as i64)])
        .collect::<Vec<_>>();
    let mut bytes = fixture::database(profile, &[4, 8], &[(1, descending)], &rows, 0);
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes[PROFILE_OFFSET] = profile;
    bytes
}

fn matches(value: &Value, lower: Bound<LegacyDateTime>, upper: Bound<LegacyDateTime>) -> bool {
    let Value::DateTime(value) = value else {
        return false;
    };
    let after = match lower {
        Bound::Unbounded => true,
        Bound::Included(bound) => !value.temporal_cmp(bound).is_lt(),
        Bound::Excluded(bound) => value.temporal_cmp(bound).is_gt(),
    };
    let before = match upper {
        Bound::Unbounded => true,
        Bound::Included(bound) => !value.temporal_cmp(bound).is_gt(),
        Bound::Excluded(bound) => value.temporal_cmp(bound).is_lt(),
    };
    after && before
}

#[test]
fn calendar_candidates_preserve_every_match_and_both_native_orders_in_priority_profiles() {
    let values = samples();
    let cases = [
        (Bound::Included(date(-0.25)), Bound::Included(date(0.25))),
        (Bound::Included(date(-0.0)), Bound::Included(date(0.0))),
        (Bound::Included(date(-1.25)), Bound::Excluded(date(-1.75))),
        (Bound::Excluded(date(-2.75)), Bound::Included(date(1.25))),
        (Bound::Unbounded, Bound::Excluded(date(-1.25))),
        (Bound::Included(date(1.0)), Bound::Unbounded),
        (Bound::Excluded(date(-0.25)), Bound::Included(date(0.25))),
        (
            Bound::Included(date(45_000.125)),
            Bound::Included(date(45_000.125)),
        ),
    ];
    for profile in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = database(profile, descending, &values);
            let full = read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            for (lower, upper) in cases {
                let selected = read_table_index_with_request(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    IndexReadRequest {
                        columns: None,
                        selection: IndexProjectionSelection::DateTime(
                            DateTimeIndexSelection::CalendarCandidates { lower, upper },
                        ),
                        maximum_rows: 100,
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let expected = full
                    .rows()
                    .iter()
                    .filter(|row| matches(&row[1], lower, upper))
                    .collect::<Vec<_>>();
                let actual = selected
                    .rows()
                    .iter()
                    .filter(|row| matches(&row[1], lower, upper))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
                assert!(
                    selected
                        .rows()
                        .iter()
                        .all(|row| !matches!(row[1], Value::Null))
                );
            }
            let nulls = read_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexReadRequest {
                    columns: Some(&["Id"]),
                    selection: IndexProjectionSelection::DateTime(DateTimeIndexSelection::Null),
                    maximum_rows: 1,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(nulls.rows(), &[vec![Value::Int32(1)]]);
        }
    }
}

#[test]
fn projected_points_cross_complete_snapshot_limits_and_table_order_is_retained() {
    let bytes = database(6, true, &samples());
    let point = date(45_000.125);
    let result = read_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns: Some(&["Id"]),
            selection: IndexProjectionSelection::DateTime(
                DateTimeIndexSelection::CalendarCandidates {
                    lower: Bound::Included(point),
                    upper: Bound::Included(point),
                },
            ),
            maximum_rows: 1,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 1);

    let table_order = read_table_index_in_table_order(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns: Some(&["Id"]),
            selection: IndexProjectionSelection::DateTime(
                DateTimeIndexSelection::CalendarCandidates {
                    lower: Bound::Unbounded,
                    upper: Bound::Unbounded,
                },
            ),
            maximum_rows: 100,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = (2..=samples().len() as i32)
        .map(|id| vec![Value::Int32(id)])
        .collect::<Vec<_>>();
    assert_eq!(table_order.rows(), expected);
}

#[test]
fn typed_date_selection_rejects_other_eight_byte_carriers_and_corrupt_selected_roots() {
    for kind in [5, 7, 0x13] {
        let bytes = fixture::database(6, &[4, kind], &[(1, false)], &[vec![Some(1), Some(0)]], 0);
        for selection in [DateTimeIndexSelection::All, DateTimeIndexSelection::Null] {
            let result = read_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexReadRequest {
                    columns: None,
                    selection: IndexProjectionSelection::DateTime(selection),
                    maximum_rows: 10,
                },
                &mut budget(),
                &|| false,
            );
            assert_eq!(result.err(), Some(InspectError::Unsupported));
        }
    }

    let compound = fixture::database(
        6,
        &[4, 8, 8],
        &[(1, false), (2, true)],
        &[vec![Some(1), Some(0), Some(0)]],
        0,
    );
    for selection in [DateTimeIndexSelection::All, DateTimeIndexSelection::Null] {
        let result = read_table_index_with_request(
            &mut Cursor::new(&compound),
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexReadRequest {
                columns: None,
                selection: IndexProjectionSelection::DateTime(selection),
                maximum_rows: 10,
            },
            &mut budget(),
            &|| false,
        );
        assert_eq!(result.err(), Some(InspectError::Unsupported));
    }

    let mut bytes = database(6, false, &[Some(0)]);
    bytes[INDEX_PAGE * PAGE_BYTES] = 0;
    let result = read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns: None,
            selection: IndexProjectionSelection::DateTime(DateTimeIndexSelection::Null),
            maximum_rows: 10,
        },
        &mut budget(),
        &|| false,
    );
    assert_eq!(result.err(), Some(InspectError::Malformed));
}

#[test]
fn request_cancellation_and_byte_limits_fail_without_a_snapshot() {
    let bytes = database(6, false, &samples());
    for cancelled in [false, true] {
        let result = read_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexReadRequest {
                columns: None,
                selection: IndexProjectionSelection::DateTime(DateTimeIndexSelection::All),
                maximum_rows: 100,
            },
            &mut ResourceBudget::new(1, 100, 1_000_000),
            &|| cancelled,
        );
        if cancelled {
            assert_eq!(result.err(), Some(InspectError::Cancelled));
        } else {
            assert!(
                matches!(result, Err(InspectError::Limit(error)) if error.resource == Resource::Bytes)
            );
        }
    }
}

#[test]
fn projected_candidates_still_decode_and_validate_a_hidden_date_key() {
    let mut bytes = database(6, false, &[Some(0)]);
    let record_bytes = size_of::<u16>() + size_of::<i32>() + size_of::<u64>() + NULL_MASK_BYTES;
    let record_start = (DATA_PAGE + 1) * PAGE_BYTES - record_bytes;
    let date_offset = record_start + size_of::<u16>() + size_of::<i32>();
    bytes[date_offset..date_offset + size_of::<u64>()]
        .copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
    let point = date(0.0);
    let result = read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns: Some(&["Id"]),
            selection: IndexProjectionSelection::DateTime(
                DateTimeIndexSelection::CalendarCandidates {
                    lower: Bound::Included(point),
                    upper: Bound::Included(point),
                },
            ),
            maximum_rows: 1,
        },
        &mut budget(),
        &|| false,
    );
    assert_eq!(result.err(), Some(InspectError::Malformed));
}

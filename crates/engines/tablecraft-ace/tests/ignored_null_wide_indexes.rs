#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "ignored_null_indexes/fixture.rs"]
mod ignored;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[allow(dead_code)]
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
mod support;
#[path = "ignored_null_wide_indexes/fixture.rs"]
mod wide;

use std::{cell::Cell, io::Cursor, ops::Bound};
use tablecraft_ace::{
    CurrencyIndexSelection, EditError, IndexCheckStatus, IndexConstraintViolation, IndexProjection,
    IndexProjectionSelection, InspectError, Int32IndexSelection, Int32Update, Int64IndexSelection,
    NativeImage, TableSnapshot, WriteBlocker, check_table, read_table_index,
    read_table_index_projection, read_table_index_range,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);
const VALUES: &[Option<i64>] = &[
    Some(i64::MIN),
    None,
    Some(-9_007_199_254_740_993),
    Some(-1),
    Some(0),
    Some(1),
    Some(9_007_199_254_740_993),
    None,
    Some(i64::MAX),
    Some(1),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 2_000_000)
}

fn projection(
    currency: bool,
    lower: Bound<i64>,
    upper: Bound<i64>,
) -> IndexProjectionSelection<'static> {
    if currency {
        IndexProjectionSelection::Currency(CurrencyIndexSelection::Range {
            lower: lower.map(Currency::from_scaled),
            upper: upper.map(Currency::from_scaled),
        })
    } else {
        IndexProjectionSelection::Int64(Int64IndexSelection::Range { lower, upper })
    }
}

fn read(
    bytes: &[u8],
    selection: IndexProjectionSelection<'_>,
) -> Result<TableSnapshot, InspectError> {
    read_table_index_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        "BigKey",
        IndexProjection {
            columns: &["Id"],
            selection,
        },
        &mut budget(),
        &|| false,
    )
}

fn eligible(currency: bool) -> &'static [u8] {
    if currency { &[1, 2, 5, 6] } else { &[5, 6] }
}

#[test]
fn ignored_wide_intervals_keep_exact_extremes_and_projection_uses_unselected_keys() {
    for currency in [false, true] {
        for &code in eligible(currency) {
            for descending in [false, true] {
                for node in [false, true] {
                    let bytes = wide::database(code, currency, descending, node, VALUES);
                    for (lower, upper) in [
                        (Bound::Unbounded, Bound::Unbounded),
                        (Bound::Included(1), Bound::Included(1)),
                        (
                            Bound::Included(-9_007_199_254_740_993),
                            Bound::Included(9_007_199_254_740_993),
                        ),
                        (Bound::Excluded(i64::MAX), Bound::Unbounded),
                        (Bound::Included(1), Bound::Included(-1)),
                    ] {
                        let actual = read(&bytes, projection(currency, lower, upper)).unwrap();
                        let mut expected: Vec<_> = VALUES
                            .iter()
                            .copied()
                            .enumerate()
                            .filter_map(|(slot, value)| {
                                let value = value?;
                                let above = match lower {
                                    Bound::Included(low) => value >= low,
                                    Bound::Excluded(low) => value > low,
                                    Bound::Unbounded => true,
                                };
                                let below = match upper {
                                    Bound::Included(high) => value <= high,
                                    Bound::Excluded(high) => value < high,
                                    Bound::Unbounded => true,
                                };
                                (above && below).then_some((value, slot as i32 + 1))
                            })
                            .collect();
                        expected.sort_by(|(left, a), (right, b)| {
                            let order = left.cmp(right);
                            (if descending { order.reverse() } else { order }).then(a.cmp(b))
                        });
                        let expected: Vec<_> = expected
                            .into_iter()
                            .map(|(_, id)| vec![Value::Int32(id)])
                            .collect();
                        assert_eq!(actual.rows(), expected);
                        assert_eq!(actual.schema().declared_rows(), VALUES.len() as u32);
                    }
                    let checked = check_table(
                        &mut Cursor::new(&bytes),
                        SCOPE,
                        "Items",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(checked.rows(), VALUES.len());
                    assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
                }
            }
        }
    }
}

#[test]
fn ignored_wide_all_null_wrong_carriers_and_secondary_int64_profiles_remain_refused() {
    for currency in [false, true] {
        for &code in eligible(currency) {
            let bytes = wide::database(code, currency, false, false, VALUES);
            assert_eq!(
                read_table_index(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    "BigKey",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
            let selections = [
                IndexProjectionSelection::All,
                if currency {
                    IndexProjectionSelection::Currency(CurrencyIndexSelection::Null)
                } else {
                    IndexProjectionSelection::Int64(Int64IndexSelection::Null)
                },
                projection(!currency, Bound::Unbounded, Bound::Unbounded),
            ];
            for selection in selections {
                assert_eq!(
                    read(&bytes, selection).err(),
                    Some(InspectError::Unsupported)
                );
            }
            assert_eq!(
                read_table_index_range(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    "BigKey",
                    Int32IndexSelection::Range {
                        lower: Bound::Unbounded,
                        upper: Bound::Unbounded
                    },
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
        }
    }
    for code in [1, 2] {
        let bytes = wide::database(code, false, false, false, VALUES);
        assert_eq!(
            read(
                &bytes,
                projection(false, Bound::Unbounded, Bound::Unbounded)
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}

#[test]
fn ignored_wide_checks_cover_empty_null_unique_required_and_corruption() {
    for currency in [false, true] {
        for &code in eligible(currency) {
            for values in [
                &[][..],
                &[None, None][..],
                &[Some(i64::MIN), Some(1), None, Some(i64::MAX)][..],
            ] {
                let mut bytes = wide::database(code, currency, false, false, values);
                bytes[wide::FLAGS] = 0x83;
                let checked = check_table(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(checked.rows(), values.len());
                assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
            }
            let mut duplicate =
                wide::database(code, currency, false, false, &[Some(1), None, Some(1)]);
            duplicate[wide::FLAGS] = 0x83;
            assert_eq!(
                check_table(
                    &mut Cursor::new(&duplicate),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                ))
            );
            duplicate[wide::FLAGS] = 0x8a;
            assert_eq!(
                check_table(
                    &mut Cursor::new(&duplicate),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::IndexConstraint(
                    IndexConstraintViolation::NullKey
                ))
            );

            let mut missing =
                wide::database(code, currency, false, false, &[Some(1), None, Some(2)]);
            missing[wide::ROOT * wide::PAGE_BYTES..(wide::ROOT + 1) * wide::PAGE_BYTES]
                .copy_from_slice(&ignored::page(
                    true,
                    &[wide::key(Some(1), 0, false)],
                    0,
                    0,
                    0,
                ));
            assert_eq!(
                check_table(
                    &mut Cursor::new(&missing),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Malformed)
            );
            missing[wide::ROOT * wide::PAGE_BYTES..(wide::ROOT + 1) * wide::PAGE_BYTES]
                .copy_from_slice(&ignored::page(
                    true,
                    &[wide::key(None, 1, false), wide::key(Some(1), 0, false)],
                    0,
                    0,
                    0,
                ));
            assert_eq!(
                check_table(
                    &mut Cursor::new(&missing),
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Malformed)
            );
        }
    }
}

#[test]
fn ignored_wide_native_mutation_refusals_survive_read_qualification() {
    for currency in [false, true] {
        for &code in eligible(currency) {
            let bytes = wide::database(code, currency, false, false, VALUES);
            for updates in [Vec::new(), vec![Int32Update::new(0, 1, 99)]] {
                let image =
                    NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                        .unwrap();
                match code {
                    1 => assert_eq!(
                        image
                            .plan_int32_updates("Items", "Id", &updates, &mut budget(), &|| false)
                            .err(),
                        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
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
                        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage)),
                    ),
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn ignored_wide_every_observed_checkpoint_and_cumulative_resource_limit_remains_typed() {
    for currency in [false, true] {
        let bytes = wide::database(5, currency, false, true, VALUES);
        let observed = Cell::new(0);
        read_table_index_projection(
            &mut Cursor::new(&bytes),
            SCOPE,
            "Items",
            "BigKey",
            IndexProjection {
                columns: &["Id"],
                selection: projection(currency, Bound::Unbounded, Bound::Unbounded),
            },
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                false
            },
        )
        .unwrap();
        for stop in 1..=observed.get() {
            let calls = Cell::new(0);
            let actual = read_table_index_projection(
                &mut Cursor::new(&bytes),
                SCOPE,
                "Items",
                "BigKey",
                IndexProjection {
                    columns: &["Id"],
                    selection: projection(currency, Bound::Unbounded, Bound::Unbounded),
                },
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                },
            );
            assert_eq!(actual.err(), Some(InspectError::Cancelled));
        }
        for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
            let mut allowance = budget();
            allowance
                .charge(resource, allowance.remaining(resource))
                .unwrap();
            assert!(matches!(
                read_table_index_projection(
                    &mut Cursor::new(&bytes),
                    SCOPE,
                    "Items",
                    "BigKey",
                    IndexProjection {
                        columns: &["Id"],
                        selection: projection(currency, Bound::Unbounded, Bound::Unbounded),
                    },
                    &mut allowance,
                    &|| false
                ),
                Err(InspectError::Limit(_))
            ));
        }
    }
}

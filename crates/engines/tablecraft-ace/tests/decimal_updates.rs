#![cfg(feature = "ace16-write-preview")]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "decimal_indexes/fixture.rs"]
mod fixture;
#[path = "decimal_composite_indexes/fixture.rs"]
mod mixed;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, WriteBlocker,
    check_table, read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Guid, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
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
fn v(coefficient: Option<i128>, scale: u8) -> Value {
    coefficient
        .map(|v| Value::Decimal(Decimal::new(v, scale).unwrap()))
        .unwrap_or(Value::Null)
}
fn run(
    bytes: &[u8],
    updates: &[FixedScalarUpdate],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_batch_updates(
        &[FixedColumnUpdates::new("Items", "Score", updates)],
        &mut budget(),
        &|| false,
    )
}
fn check(bytes: &[u8]) -> Vec<Vec<Value>> {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|i| i.status() == IndexCheckStatus::Checked)
    );
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}
#[test]
fn exact_coefficients_scales_and_null_clearing_reuse_existing_variable_slots() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (precision, scale, old, new) in [
                (28, 4, 9007199254740993, (10_i128.pow(28) - 1)),
                (28, 28, -1, 1),
                (3, 2, -999, 999),
                (1, 0, -1, 0),
            ] {
                let bytes = fixture::database_for(
                    code,
                    descending,
                    precision,
                    scale,
                    &[Some(old), Some(0), None],
                    0,
                );
                let candidate = run(
                    &bytes,
                    &[FixedScalarUpdate::new(
                        0,
                        v(Some(old), scale),
                        v(Some(new), scale),
                    )],
                )
                .unwrap();
                assert_eq!(
                    check(candidate.candidate_bytes())[0][1],
                    v(Some(new), scale)
                );
                assert_eq!(candidate.changed_rows(), 1);
                let cleared = run(
                    candidate.candidate_bytes(),
                    &[FixedScalarUpdate::new(0, v(Some(new), scale), Value::Null)],
                )
                .unwrap();
                assert_eq!(check(cleared.candidate_bytes())[0][1], Value::Null);
                let restored = run(
                    cleared.candidate_bytes(),
                    &[FixedScalarUpdate::new(0, Value::Null, v(Some(new), scale))],
                )
                .unwrap();
                assert_eq!(restored.candidate_bytes(), candidate.candidate_bytes());
            }
        }
    }
}
#[test]
fn scale_precision_stale_duplicate_and_resize_requirements_reject_without_candidates() {
    let bytes = fixture::database(false, 3, 2, &[Some(1), None], 0);
    for update in [
        FixedScalarUpdate::new(0, v(Some(1), 2), v(Some(1), 1)),
        FixedScalarUpdate::new(0, v(Some(1), 2), v(Some(1000), 2)),
        FixedScalarUpdate::new(0, v(Some(2), 2), v(Some(3), 2)),
    ] {
        assert!(run(&bytes, &[update]).is_err());
    }
    assert_eq!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(1, Value::Null, v(Some(1), 2))]
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    let make = || FixedScalarUpdate::new(0, v(Some(1), 2), v(Some(3), 2));
    assert_eq!(
        run(&bytes, &[make(), make()]).err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        run(
            &bytes,
            &[
                FixedScalarUpdate::new(0, v(Some(1), 2), v(Some(1), 2)),
                FixedScalarUpdate::new(1, Value::Null, Value::Null)
            ]
        )
        .unwrap()
        .candidate_bytes(),
        bytes
    );
    let jet = fixture::database_for(1, false, 3, 2, &[Some(1)], 0);
    assert_eq!(
        run(&jet, &[make()]).err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );
}
#[test]
fn decimal_unique_swaps_validate_the_complete_index_once() {
    let mut bytes = fixture::database(false, 28, 4, &[Some(1), Some(-1), None], 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    assert!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(0, v(Some(1), 4), v(Some(-1), 4))]
        )
        .is_err()
    );
    let candidate = run(
        &bytes,
        &[
            FixedScalarUpdate::new(0, v(Some(1), 4), v(Some(-1), 4)),
            FixedScalarUpdate::new(1, v(Some(-1), 4), v(Some(1), 4)),
        ],
    )
    .unwrap();
    assert_eq!(check(candidate.candidate_bytes())[0][1], v(Some(-1), 4));
    assert_eq!(candidate.changed_rows(), 2);
}
#[test]
fn decimal_components_in_two_to_ten_column_keys_keep_exact_siblings_and_nulls() {
    use mixed::Cell::{Decimal as D, Guid as G};
    for count in 2..=10 {
        for target in 0..count {
            let kinds = (0..count)
                .map(|p| if p == target { 16 } else { 15 })
                .collect::<Vec<_>>();
            let order = (0..count).map(|p| (p, p % 2 != 0)).collect::<Vec<_>>();
            let rows = (0..3)
                .map(|row| {
                    (0..count)
                        .map(|p| {
                            if p == target {
                                Some(D(row as i128 + 1))
                            } else {
                                Some(G([row as u8 + 1; 16]))
                            }
                        })
                        .collect()
                })
                .collect::<Vec<_>>();
            let bytes = mixed::database(6, &kinds, &order, &rows, 0);
            let names = [
                "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
            ];
            let decimal = [FixedScalarUpdate::new(
                0,
                v(Some(1), 4),
                v(Some(-9007199254740993), 4),
            )];
            let sibling = (target + 1) % count;
            let guid = [FixedScalarUpdate::new(
                0,
                Value::Guid(Guid::from_bytes([1; 16])),
                Value::Guid(Guid::from_bytes([9; 16])),
            )];
            let candidate = capture(&bytes)
                .preview_ace16_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", names[target], &decimal),
                        FixedColumnUpdates::new("Items", names[sibling], &guid),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let rows = check(candidate.candidate_bytes());
            assert_eq!(rows[0][target], v(Some(-9007199254740993), 4));
            assert_eq!(candidate.changed_rows(), 1);
        }
    }
}
#[test]
fn budgets_and_cancellation_return_no_candidate() {
    let bytes = fixture::database(false, 28, 4, &[Some(1)], 0);
    let changes = [FixedScalarUpdate::new(0, v(Some(1), 4), v(Some(2), 4))];
    for mut b in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&bytes)
                .preview_ace16_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Score", &changes)],
                    &mut b,
                    &|| false
                )
                .is_err()
        );
    }
    for limit in [0, 50, 300] {
        let calls = Cell::new(0);
        assert!(
            capture(&bytes)
                .preview_ace16_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Score", &changes)],
                    &mut budget(),
                    &|| {
                        let n = calls.get();
                        calls.set(n + 1);
                        n >= limit
                    }
                )
                .is_err()
        );
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4445435752495445),
        ..Default::default()
    })]
    #[test]
    fn coefficients_remain_exact_across_full_width_variable_slot_replacements(
        old in -(10_i128.pow(28) - 1)..=(10_i128.pow(28) - 1),
        new in -(10_i128.pow(28) - 1)..=(10_i128.pow(28) - 1),
        descending in any::<bool>(),
    ) {
        let bytes = fixture::database(descending, 28, 4, &[Some(old), None], 0);
        let candidate = run(&bytes, &[
            FixedScalarUpdate::new(0, v(Some(old), 4), v(Some(new), 4))
        ]).unwrap();
        prop_assert_eq!(&check(candidate.candidate_bytes())[0][1], &v(Some(new), 4));
    }
}

#[test]
fn noncanonical_stored_negative_zero_is_not_silently_normalized_by_an_edit() {
    let mut bytes = fixture::database(false, 28, 4, &[Some(0)], 0);
    let page = 12 * 4096;
    let start =
        usize::from(u16::from_le_bytes(bytes[page + 14..page + 16].try_into().unwrap()) & 0x1fff);
    // Current authored record: two-byte count, four-byte Id, Decimal variable slot.
    bytes[page + start + 2 + 4] = 0x80;
    assert_eq!(check(&bytes)[0][1], v(Some(0), 4));
    assert_eq!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(0, v(Some(0), 4), v(Some(1), 4))]
        )
        .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn wider_expression_decimals_cannot_enter_28_digit_native_fields() {
    for code in [5, 6] {
        let bytes = fixture::database_for(code, false, 28, 0, &[Some(1)], 0);
        for coefficient in [
            10_i128.pow(28),
            -10_i128.pow(28),
            Decimal::MAX_COEFFICIENT,
            -Decimal::MAX_COEFFICIENT,
        ] {
            let original = bytes.clone();
            assert!(
                run(
                    &bytes,
                    &[FixedScalarUpdate::new(
                        0,
                        v(Some(1), 0),
                        v(Some(coefficient), 0)
                    )]
                )
                .is_err()
            );
            assert_eq!(bytes, original);
        }
    }
}

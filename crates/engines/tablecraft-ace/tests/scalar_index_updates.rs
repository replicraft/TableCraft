#![cfg(feature = "ace16-write-preview")]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "guid_composite_indexes/fixture.rs"]
mod mixed;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, check_table,
    read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, Guid, LegacyDateTime, Single, Value},
};
use tablecraft_model::CatalogScope;

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
fn carrier(kind: u8, bits: Option<u64>) -> Value {
    match bits {
        None => Value::Null,
        Some(bits) => match kind {
            6 => Value::Single(Single::from_bits(bits as u32)),
            7 => Value::Double(Double::from_bits(bits)),
            8 => Value::DateTime(LegacyDateTime::from_bits(bits).unwrap()),
            _ => panic!("carrier"),
        },
    }
}
fn database(code: u8, kind: u8, descending: bool, values: &[Option<u64>]) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, v)| vec![Some(i as i64 + 1), v.map(|v| v as i64)])
        .collect::<Vec<_>>();
    numeric_fixture::database(code, &[4, kind], &[(1, descending)], &rows, 0)
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
fn booleans_change_the_mask_and_keep_false_as_a_real_index_key() {
    for code in [5, 6] {
        for descending in [false, true] {
            let rows = vec![vec![Some(1), Some(0)], vec![Some(2), Some(-1)]];
            let mut bytes = numeric_fixture::database(code, &[4, 1], &[(1, descending)], &rows, 0);
            numeric_fixture::index_flags(&mut bytes, 0x81);
            let updates = [
                FixedScalarUpdate::new(0, Value::Boolean(false), Value::Boolean(true)),
                FixedScalarUpdate::new(1, Value::Boolean(true), Value::Boolean(false)),
            ];
            let candidate = run(&bytes, &updates).unwrap();
            let final_rows = check(candidate.candidate_bytes());
            assert_eq!(final_rows[0][1], Value::Boolean(true));
            assert_eq!(final_rows[1][1], Value::Boolean(false));
            assert_eq!(candidate.changed_rows(), 2);
            assert!(run(&bytes, &updates[..1]).is_err());
            assert!(
                run(
                    &bytes,
                    &[FixedScalarUpdate::new(
                        0,
                        Value::Boolean(false),
                        Value::Null
                    )]
                )
                .is_err()
            );
        }
    }
}
#[test]
fn ieee_payloads_signed_zero_and_null_transitions_are_exact() {
    for code in [5, 6] {
        for descending in [false, true] {
            for kind in [6, 7] {
                let patterns = if kind == 6 {
                    vec![
                        0, 0x80000000, 1, 0x7f800000, 0xff800000, 0x7fc00001, 0xffc00002,
                        0x3f800001,
                    ]
                } else {
                    vec![
                        0,
                        0x8000000000000000,
                        1,
                        0x7ff0000000000000,
                        0xfff0000000000000,
                        0x7ff8000000000001,
                        0xfff8000000000002,
                        0x3ff0000000000001,
                    ]
                };
                for &new in &patterns {
                    let bytes = database(code, kind, descending, &[Some(0), None, Some(1)]);
                    let candidate = run(
                        &bytes,
                        &[
                            FixedScalarUpdate::new(
                                0,
                                carrier(kind, Some(0)),
                                carrier(kind, Some(new)),
                            ),
                            FixedScalarUpdate::new(1, Value::Null, carrier(kind, Some(new))),
                            FixedScalarUpdate::new(2, carrier(kind, Some(1)), Value::Null),
                        ],
                    )
                    .unwrap();
                    let rows = check(candidate.candidate_bytes());
                    assert_eq!(rows[0][1], carrier(kind, Some(new)));
                    assert_eq!(rows[1][1], carrier(kind, Some(new)));
                    assert_eq!(rows[2][1], Value::Null);
                }
            }
        }
    }
}
#[test]
fn date_aliases_submillisecond_bits_and_zero_signs_do_not_round_trip_through_calendars() {
    for code in [5, 6] {
        for descending in [false, true] {
            let bits = [
                0,
                (-0.0_f64).to_bits(),
                0.25_f64.to_bits(),
                (-0.25_f64).to_bits(),
                1.0000000001_f64.to_bits(),
                (-1.0000000001_f64).to_bits(),
            ];
            for &new in &bits {
                let bytes = database(code, 8, descending, &[Some(0), None, Some(1)]);
                let candidate = run(
                    &bytes,
                    &[
                        FixedScalarUpdate::new(0, carrier(8, Some(0)), carrier(8, Some(new))),
                        FixedScalarUpdate::new(1, Value::Null, carrier(8, Some(new))),
                        FixedScalarUpdate::new(2, carrier(8, Some(1)), Value::Null),
                    ],
                )
                .unwrap();
                let rows = check(candidate.candidate_bytes());
                assert_eq!(rows[0][1], carrier(8, Some(new)));
                assert_eq!(rows[1][1], carrier(8, Some(new)));
                assert_eq!(rows[2][1], Value::Null);
            }
        }
    }
}
#[test]
fn full_mixed_tuples_merge_boolean_masks_with_ieee_date_guid_and_integer_changes() {
    use mixed::Cell::{Guid as G, Number as N};
    let kinds = [1, 6, 7, 8, 15, 5, 2, 3, 4, 1];
    let original = vec![
        vec![
            Some(N(0)),
            Some(N(0)),
            Some(N(0)),
            Some(N(0)),
            Some(G([1; 16])),
            Some(N(1)),
            Some(N(1)),
            Some(N(1)),
            Some(N(1)),
            Some(N(-1)),
        ],
        vec![
            Some(N(-1)),
            Some(N(1)),
            Some(N(1)),
            Some(N(1)),
            Some(G([2; 16])),
            Some(N(2)),
            Some(N(2)),
            Some(N(2)),
            Some(N(2)),
            Some(N(0)),
        ],
    ];
    let order = (0..10).map(|p| (p, p % 2 != 0)).collect::<Vec<_>>();
    let bytes = mixed::database(6, &kinds, &order, &original, 0);
    let old = check(&bytes);
    let new = [
        Value::Boolean(true),
        carrier(6, Some(0xffc00002)),
        carrier(7, Some(0x8000000000000000)),
        carrier(8, Some((-0.25_f64).to_bits())),
        Value::Guid(Guid::from_bytes([9; 16])),
        Value::Currency(Currency::from_scaled(9007199254740993)),
        Value::Byte(255),
        Value::Int16(-32768),
        Value::Int32(-1),
        Value::Boolean(false),
    ];
    let names = [
        "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
    ];
    let updates = (0..10)
        .map(|p| [FixedScalarUpdate::new(0, old[0][p].clone(), new[p].clone())])
        .collect::<Vec<_>>();
    let groups = (0..10)
        .map(|p| FixedColumnUpdates::new("Items", names[p], &updates[p]))
        .collect::<Vec<_>>();
    let candidate = capture(&bytes)
        .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(check(candidate.candidate_bytes())[0], new);
    assert_eq!(candidate.changed_rows(), 1);
    let reverse = groups.into_iter().rev().collect::<Vec<_>>();
    assert_eq!(
        candidate.candidate_bytes(),
        capture(&bytes)
            .preview_ace16_batch_updates(&reverse, &mut budget(), &|| false)
            .unwrap()
            .candidate_bytes()
    );
}
#[test]
fn stale_duplicates_budget_and_cancellation_fail_without_a_candidate() {
    let bytes = database(6, 7, false, &[Some(0), None]);
    let change = || FixedScalarUpdate::new(0, carrier(7, Some(0)), carrier(7, Some(1)));
    assert_eq!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(
                0,
                carrier(7, Some(2)),
                carrier(7, Some(1))
            )]
        )
        .err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        run(&bytes, &[change(), change()]).err(),
        Some(EditError::Conflict)
    );
    for mut b in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&bytes)
                .preview_ace16_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Score", &[change()])],
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
                    &[FixedColumnUpdates::new("Items", "Score", &[change()])],
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
 #![proptest_config(ProptestConfig{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4945454557524954),..Default::default()})]
 #[test]
 fn arbitrary_ieee_replacements_match_exact_original_carriers(old in any::<u64>(),new in any::<u64>(),descending in any::<bool>()){
  let bytes=database(6,7,descending,&[Some(old),None]);let candidate=run(&bytes,&[FixedScalarUpdate::new(0,carrier(7,Some(old)),carrier(7,Some(new)))]).unwrap();prop_assert_eq!(&check(candidate.candidate_bytes())[0][1],&carrier(7,Some(new)));
 }
}

#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod base;
#[path = "composite_updates/capacity.rs"]
mod capacity;
#[path = "composite_updates/model.rs"]
mod model;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;
#[path = "composite_updates/trees.rs"]
mod trees;

use model::{name, value, verify};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexCheckStatus, NativeImage, check_table, read_table_index,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
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
fn preview(
    bytes: &[u8],
    kinds: &[u8],
    column: usize,
    row: usize,
    old: Option<i64>,
    new: Option<i64>,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_fixed_updates(
        "Items",
        name(column),
        &[FixedScalarUpdate::new(
            row,
            value(kinds[column], old),
            value(kinds[column], new),
        )],
        &mut budget(),
        &|| false,
    )
}

#[test]
fn two_to_ten_mixed_integral_columns_keep_directions_nulls_aliases_and_exact_values() {
    for code in [5, 6] {
        for count in 2..=10 {
            for reverse in [false, true] {
                let kinds = (0..count)
                    .map(|i| [2, 3, 4, 0x13, 5][i % 5])
                    .collect::<Vec<_>>();
                let order = (0..count)
                    .rev()
                    .map(|i| (i, (i % 2 == 0) ^ reverse))
                    .collect::<Vec<_>>();
                let mut rows = vec![
                    vec![Some(10); count],
                    vec![Some(20); count],
                    vec![None; count],
                ];
                for i in (0..count).step_by(2) {
                    rows[0][i] = None;
                }
                let bytes = model::with_alias(
                    numeric_fixture::database(code, &kinds, &order, &rows, 0),
                    count,
                );
                for column in 0..count {
                    let mut expected = rows.clone();
                    expected[1][column] = None;
                    let candidate = preview(&bytes, &kinds, column, 1, Some(20), None).unwrap();
                    verify(
                        &bytes,
                        candidate.candidate_bytes(),
                        &kinds,
                        &order,
                        &expected,
                        column,
                        &[1],
                    );
                    let restored = preview(
                        candidate.candidate_bytes(),
                        &kinds,
                        column,
                        1,
                        None,
                        Some(20),
                    )
                    .unwrap();
                    verify(
                        candidate.candidate_bytes(),
                        restored.candidate_bytes(),
                        &kinds,
                        &order,
                        &rows,
                        column,
                        &[1],
                    );
                }
            }
        }
    }
}

#[test]
fn tuple_uniqueness_requiredness_all_null_canonicalization_and_batch_swaps_are_exact() {
    let kinds = [4, 0x13];
    let order = [(0, false), (1, true)];
    let rows = vec![
        vec![Some(1), Some(10)],
        vec![Some(1), Some(20)],
        vec![None, None],
        vec![None, None],
    ];
    let mut bytes = numeric_fixture::database(6, &kinds, &order, &rows, 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    assert!(preview(&bytes, &kinds, 1, 0, Some(10), Some(20)).is_err());
    let swapped = capture(&bytes)
        .preview_ace16_fixed_updates(
            "Items",
            "Score",
            &[
                FixedScalarUpdate::new(0, Value::Int64(10), Value::Int64(20)),
                FixedScalarUpdate::new(1, Value::Int64(20), Value::Int64(10)),
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let mut expected = rows.clone();
    expected[0][1] = Some(20);
    expected[1][1] = Some(10);
    verify(
        &bytes,
        swapped.candidate_bytes(),
        &kinds,
        &order,
        &expected,
        1,
        &[0, 1],
    );
    let all_null = vec![vec![Some(1), None], vec![None, None], vec![None, None]];
    let mut bytes = numeric_fixture::database(6, &kinds, &order, &all_null, 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    let cleared = preview(&bytes, &kinds, 0, 0, Some(1), None).unwrap();
    verify(
        &bytes,
        cleared.candidate_bytes(),
        &kinds,
        &order,
        &vec![vec![None, None]; 3],
        0,
        &[0],
    );
    let partial = vec![
        vec![None, None],
        vec![Some(1), None],
        vec![Some(1), Some(2)],
    ];
    let mut bytes = numeric_fixture::database(6, &kinds, &order, &partial, 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    assert!(preview(&bytes, &kinds, 0, 0, None, Some(1)).is_err());
    let mut bytes = numeric_fixture::database(6, &kinds, &order, &rows[..2], 0);
    numeric_fixture::index_flags(&mut bytes, 0x88);
    assert!(preview(&bytes, &kinds, 1, 0, Some(10), None).is_err());
}

#[test]
fn compression_noops_and_scalar_siblings_preserve_existing_values() {
    let kinds = [4, 3];
    let order = [(0, false), (1, true)];
    let rows = vec![vec![Some(1), Some(10)], vec![Some(1), Some(20)]];
    let bytes = numeric_fixture::database(6, &kinds, &order, &rows, 5);
    let unchanged = preview(&bytes, &kinds, 1, 0, Some(10), Some(10)).unwrap();
    assert_eq!(unchanged.candidate_bytes(), bytes);
    assert_eq!(unchanged.changed_rows(), 0);
    let candidate = preview(&bytes, &kinds, 0, 0, Some(1), Some(2)).unwrap();
    let mut expected = rows.clone();
    expected[0][0] = Some(2);
    verify(
        &bytes,
        candidate.candidate_bytes(),
        &kinds,
        &order,
        &expected,
        0,
        &[0],
    );
    for other in [1, 6, 7, 8] {
        let bytes = numeric_fixture::database(
            6,
            &[4, other],
            &order,
            &[vec![Some(1), Some(0)], vec![Some(2), Some(0)]],
            0,
        );
        let original = read_table_index(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let candidate = preview(&bytes, &[4, other], 0, 0, Some(1), Some(3)).unwrap();
        let report = check_table(
            &mut Cursor::new(candidate.candidate_bytes()),
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
        let table = read_table_index(
            &mut Cursor::new(candidate.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.rows()[0][0], Value::Int32(2));
        assert_eq!(table.rows()[1][0], Value::Int32(3));
        assert_eq!(table.rows()[0][1], original.rows()[1][1]);
        assert_eq!(table.rows()[1][1], original.rows()[0][1]);
    }
}

#[test]
fn cancellation_and_budgets_reject_complete_tuple_candidates() {
    let kinds = [4, 0x13];
    let order = [(0, false), (1, true)];
    let rows = vec![vec![Some(1), Some(i64::MIN)], vec![Some(1), Some(i64::MAX)]];
    let bytes = numeric_fixture::database(6, &kinds, &order, &rows, 0);
    let calls = Cell::new(0);
    let run = |stop| {
        capture(&bytes).preview_ace16_fixed_updates(
            "Items",
            "Score",
            &[FixedScalarUpdate::new(
                0,
                Value::Int64(i64::MIN),
                Value::Null,
            )],
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        )
    };
    run(usize::MAX).unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert!(run(stop).is_err());
    }
    assert!(
        capture(&bytes)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(
                    0,
                    Value::Int64(i64::MIN),
                    Value::Null
                )],
                &mut ResourceBudget::new(0, 100_000, 10_000_000),
                &|| false
            )
            .is_err()
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x5455504c45454449),..Default::default()})]
    #[test]
    fn exact_nullable_tuples_match_an_independent_lexicographic_model(a in prop::option::of(any::<i64>()),b in prop::option::of(any::<i64>()),c in prop::option::of(any::<i64>()),dirs in any::<[bool;3]>()) {
        let kinds=[4,0x13,5];let order=[(2,dirs[0]),(0,dirs[1]),(1,dirs[2])];
        let rows=vec![vec![None,a,b],vec![Some(1),b,c],vec![Some(1),None,a]];
        let bytes=numeric_fixture::database(6,&kinds,&order,&rows,0);let candidate=preview(&bytes,&kinds,1,0,a,c).unwrap();
        let mut expected=rows;expected[0][1]=c;
        verify(&bytes,candidate.candidate_bytes(),&kinds,&order,&expected,1,&[0]);
        prop_assert_eq!(candidate.changed_rows(),usize::from(a!=c));
    }
}

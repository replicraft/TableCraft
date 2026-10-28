#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod base;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;
#[path = "integral_index_updates/trees.rs"]
mod trees;

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
const ENTRY_START: usize = 480;

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
fn value(kind: u8, key: Option<i64>) -> Value {
    match key {
        None => Value::Null,
        Some(key) => match kind {
            2 => Value::Byte(u8::try_from(key).unwrap()),
            3 => Value::Int16(i16::try_from(key).unwrap()),
            4 => Value::Int32(i32::try_from(key).unwrap()),
            0x13 => Value::Int64(key),
            5 => Value::Currency(Currency::from_scaled(key)),
            _ => panic!("test carrier"),
        },
    }
}
fn source(code: u8, kind: u8, descending: bool, keys: &[Option<i64>], prefix: usize) -> Vec<u8> {
    let rows = keys
        .iter()
        .enumerate()
        .map(|(row, key)| vec![Some(row as i64 + 1), *key])
        .collect::<Vec<_>>();
    numeric_fixture::database(code, &[4, kind], &[(1, descending)], &rows, prefix)
}
fn preview(
    bytes: &[u8],
    kind: u8,
    before: Option<i64>,
    after: Option<i64>,
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_fixed_updates(
        "Items",
        "Score",
        &[FixedScalarUpdate::new(
            0,
            value(kind, before),
            value(kind, after),
        )],
        &mut budget(),
        &|| false,
    )
}
fn verify(before: &[u8], after: &[u8], kind: u8, descending: bool, keys: &[Option<i64>]) {
    let mut model = keys.iter().copied().enumerate().collect::<Vec<_>>();
    model.sort_by(|a, b| {
        let cmp = a.1.cmp(&b.1);
        (if descending { cmp.reverse() } else { cmp }).then(a.0.cmp(&b.0))
    });
    let table = read_table_index(
        &mut Cursor::new(after),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (actual, (ordinal, key)) in table.rows().iter().zip(&model) {
        assert_eq!(actual[0], Value::Int32(*ordinal as i32 + 1));
        assert_eq!(actual[1], value(kind, *key));
    }
    assert_eq!(table.rows().len(), model.len());
    assert!(
        check_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .iter()
        .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(before.len(), after.len());
    // Independent unchanged-byte proof: one fixed field/mask and admitted index
    // regions only. All records, links, stamps, and unused page suffixes survive.
    let width = match kind {
        2 => 1,
        3 => 2,
        4 => 4,
        _ => 8,
    };
    let row = 12 * PAGE_BYTES
        + (u16::from_le_bytes(
            before[12 * PAGE_BYTES + 14..12 * PAGE_BYTES + 16]
                .try_into()
                .unwrap(),
        ) as usize
            & 0x1fff);
    let mut allowed = vec![false; before.len()];
    allowed[row + 6..row + 6 + width].fill(true);
    allowed[row + 6 + width] = true;
    assert_eq!((before[row + 6 + width] ^ after[row + 6 + width]) & !2, 0);
    if keys[0].is_none() {
        assert_eq!(
            &before[row + 6..row + 6 + width],
            &after[row + 6..row + 6 + width]
        );
    }
    for (number, page) in before.as_chunks::<PAGE_BYTES>().0.iter().enumerate() {
        if matches!(page[0], 3 | 4) && u32::from_le_bytes(page[4..8].try_into().unwrap()) == 10 {
            let start = number * PAGE_BYTES;
            let old = PAGE_BYTES - u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
            let new = PAGE_BYTES
                - u16::from_le_bytes(after[start + 2..start + 4].try_into().unwrap()) as usize;
            assert!(new >= ENTRY_START);
            allowed[start + 2..start + 4].fill(true);
            allowed[start + 24..start + 26].fill(true);
            allowed[start + 27..start + old.max(new)].fill(true);
        }
    }
    assert!(
        before
            .iter()
            .zip(after)
            .zip(allowed)
            .all(|((&a, &b), allowed)| a == b || allowed)
    );
}

#[test]
fn exact_integral_domains_move_extremes_duplicates_and_nulls_without_rounding() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (kind, min, max) in [
                (2, 0, 255),
                (3, i16::MIN as i64, i16::MAX as i64),
                (4, i32::MIN as i64, i32::MAX as i64),
                (0x13, i64::MIN, i64::MAX),
                (5, i64::MIN, i64::MAX),
            ] {
                let keys = [Some(min), Some(max), Some(max), None];
                let bytes = source(code, kind, descending, &keys, 0);
                let candidate = preview(&bytes, kind, Some(min), Some(max)).unwrap();
                verify(
                    &bytes,
                    candidate.candidate_bytes(),
                    kind,
                    descending,
                    &[Some(max), Some(max), Some(max), None],
                );
                let cleared = preview(candidate.candidate_bytes(), kind, Some(max), None).unwrap();
                verify(
                    candidate.candidate_bytes(),
                    cleared.candidate_bytes(),
                    kind,
                    descending,
                    &[None, Some(max), Some(max), None],
                );
                let restored = preview(cleared.candidate_bytes(), kind, None, Some(min)).unwrap();
                verify(
                    cleared.candidate_bytes(),
                    restored.candidate_bytes(),
                    kind,
                    descending,
                    &keys,
                );
                let noop = preview(&bytes, kind, Some(min), Some(min)).unwrap();
                assert_eq!(noop.candidate_bytes(), bytes);
                assert_eq!(noop.changed_rows(), 0);
            }
        }
    }
}

#[test]
fn compressed_fixed_updates_keep_the_original_layout_and_reject_prefix_changes() {
    for kind in [2, 3, 0x13, 5] {
        let prefix = match kind {
            2 => 1,
            3 => 2,
            _ => 8,
        };
        let bytes = source(6, kind, false, &[Some(10), Some(20), Some(30)], prefix);
        let candidate = preview(&bytes, kind, Some(10), Some(25)).unwrap();
        verify(
            &bytes,
            candidate.candidate_bytes(),
            kind,
            false,
            &[Some(25), Some(20), Some(30)],
        );
        assert_eq!(
            &bytes[14 * PAGE_BYTES..14 * PAGE_BYTES + ENTRY_START],
            &candidate.candidate_bytes()[14 * PAGE_BYTES..14 * PAGE_BYTES + ENTRY_START]
        );
        if kind != 2 {
            assert!(preview(&bytes, kind, Some(10), Some(-1)).is_err());
        }
        let candidate = preview(&bytes, kind, Some(10), None).unwrap();
        verify(
            &bytes,
            candidate.candidate_bytes(),
            kind,
            false,
            &[None, Some(20), Some(30)],
        );
    }
}

#[test]
fn uniqueness_stale_values_wrong_carriers_and_late_cancellation_reject_without_candidates() {
    for kind in [2, 3, 0x13, 5] {
        let mut bytes = source(6, kind, false, &[Some(10), Some(20)], 0);
        numeric_fixture::index_flags(&mut bytes, 0x81);
        assert!(preview(&bytes, kind, Some(10), Some(20)).is_err());
        // Validate uniqueness after the complete batch, so an exact key swap
        // succeeds without transient duplicate rejection or lossy widening.
        let swapped = capture(&bytes)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[
                    FixedScalarUpdate::new(0, value(kind, Some(10)), value(kind, Some(20))),
                    FixedScalarUpdate::new(1, value(kind, Some(20)), value(kind, Some(10))),
                ],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let rows = read_table_index(
            &mut Cursor::new(swapped.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows(),
            &[
                vec![Value::Int32(2), value(kind, Some(10))],
                vec![Value::Int32(1), value(kind, Some(20))]
            ]
        );
        assert_eq!(swapped.changed_rows(), 2);
        assert!(preview(&bytes, kind, Some(11), Some(30)).is_err());
        assert!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &[FixedScalarUpdate::new(
                        0,
                        Value::Int32(10),
                        Value::Int32(30)
                    )],
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        let calls = Cell::new(0);
        let run = |cancel_at| {
            capture(&bytes).preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(
                    0,
                    value(kind, Some(10)),
                    value(kind, None),
                )],
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == cancel_at
                },
            )
        };
        run(usize::MAX).unwrap();
        let total = calls.get();
        calls.set(0);
        assert!(run(total).is_err());
        assert!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &[FixedScalarUpdate::new(
                        0,
                        value(kind, Some(10)),
                        Value::Null
                    )],
                    &mut ResourceBudget::new(0, 100_000, 10_000_000),
                    &|| false
                )
                .is_err()
        );
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x494e54454752414c),..Default::default()})]
    #[test]
    fn large_number_and_currency_match_an_exact_nullable_integer_model(a in prop::option::of(any::<i64>()),b in prop::option::of(any::<i64>()),descending in any::<bool>(),currency in any::<bool>()) {
        let kind=if currency{5}else{0x13};
        let bytes=source(6,kind,descending,&[a,b,Some(0)],0);
        let candidate=preview(&bytes,kind,a,b).unwrap();
        verify(&bytes,candidate.candidate_bytes(),kind,descending,&[b,b,Some(0)]);
    }
}

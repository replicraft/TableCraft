#![cfg(feature = "ace16-write-preview")]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "guid_indexes/fixture.rs"]
mod guid_fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;
#[path = "guid_composite_indexes/fixture.rs"]
mod tuple_fixture;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, NativeImage, check_table,
    read_table, read_table_index,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
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
fn value(bytes: Option<[u8; Guid::BYTE_LEN]>) -> Value {
    bytes
        .map(|v| Value::Guid(Guid::from_bytes(v)))
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
fn checked(bytes: &[u8]) {
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
}
#[test]
fn guid_reordering_and_null_transitions_keep_stored_canonical_bytes() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (old, new) in [
                (Some([1; 16]), Some([9; 16])),
                (Some([1; 16]), None),
                (None, Some([9; 16])),
            ] {
                let mut original = vec![old, Some([2; 16]), None, Some([3; 16])];
                let bytes = guid_fixture::database(code, descending, &original, 0);
                let candidate =
                    run(&bytes, &[FixedScalarUpdate::new(0, value(old), value(new))]).unwrap();
                assert_eq!(candidate.changed_rows(), 1);
                checked(candidate.candidate_bytes());
                original[0] = new;
                let sequential = read_table(
                    &mut Cursor::new(candidate.candidate_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    sequential
                        .rows()
                        .iter()
                        .map(|r| r[1].clone())
                        .collect::<Vec<_>>(),
                    original.into_iter().map(value).collect::<Vec<_>>()
                );
                let ordered = read_table_index(
                    &mut Cursor::new(candidate.candidate_bytes()),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(ordered.rows().len(), 4);
            }
        }
    }
}
#[test]
fn unique_guid_swaps_and_duplicate_rejections_are_checked_on_the_complete_batch() {
    let mut bytes = guid_fixture::database(6, false, &[Some([1; 16]), Some([2; 16]), None], 0);
    // The authored fixture has one ordinary definition at the documented offset.
    numeric_fixture::index_flags(&mut bytes, 0x81);
    checked(&bytes);
    let duplicate = [FixedScalarUpdate::new(
        0,
        value(Some([1; 16])),
        value(Some([2; 16])),
    )];
    assert!(run(&bytes, &duplicate).is_err());
    let swap = [
        FixedScalarUpdate::new(0, value(Some([1; 16])), value(Some([2; 16]))),
        FixedScalarUpdate::new(1, value(Some([2; 16])), value(Some([1; 16]))),
    ];
    let result = run(&bytes, &swap).unwrap();
    checked(result.candidate_bytes());
    assert_eq!(result.changed_rows(), 2);
}
#[test]
fn guid_tuple_components_and_numeric_siblings_share_one_final_index_rewrite() {
    use tuple_fixture::Cell::{Guid as G, Number as N};
    for count in 2..=10 {
        for target in 0..count {
            let kinds = (0..count)
                .map(|p| if p == target { 15 } else { 4 })
                .collect::<Vec<_>>();
            let order = (0..count).map(|p| (p, p % 2 != 0)).collect::<Vec<_>>();
            let rows = (0..3)
                .map(|row| {
                    (0..count)
                        .map(|p| {
                            if p == target {
                                Some(G([row as u8 + 1; 16]))
                            } else {
                                Some(N(row + 1))
                            }
                        })
                        .collect()
                })
                .collect::<Vec<_>>();
            let bytes = tuple_fixture::database(6, &kinds, &order, &rows, 0);
            let names = [
                "Id", "Score", "K2", "K3", "K4", "K5", "K6", "K7", "K8", "K9",
            ];
            let guid = [FixedScalarUpdate::new(
                0,
                value(Some([1; 16])),
                value(Some([9; 16])),
            )];
            let sibling = (target + 1) % count;
            let numeric = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(19))];
            let candidate = capture(&bytes)
                .preview_ace16_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", names[target], &guid),
                        FixedColumnUpdates::new("Items", names[sibling], &numeric),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            checked(candidate.candidate_bytes());
            assert_eq!(candidate.changed_rows(), 1);
        }
    }
}
#[test]
fn stale_duplicate_noop_and_resource_failures_publish_no_candidate() {
    let bytes = guid_fixture::database(6, false, &[Some([1; 16]), Some([2; 16])], 0);
    let change = || FixedScalarUpdate::new(0, value(Some([1; 16])), value(Some([9; 16])));
    assert_eq!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(
                0,
                value(Some([2; 16])),
                value(Some([9; 16]))
            )]
        )
        .err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        run(&bytes, &[change(), change()]).err(),
        Some(EditError::Conflict)
    );
    assert_eq!(
        run(
            &bytes,
            &[FixedScalarUpdate::new(
                0,
                value(Some([1; 16])),
                value(Some([1; 16]))
            )]
        )
        .unwrap()
        .candidate_bytes(),
        bytes
    );
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&bytes)
                .preview_ace16_batch_updates(
                    &[FixedColumnUpdates::new("Items", "Score", &[change()])],
                    &mut budget,
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
    #![proptest_config(ProptestConfig{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4755494457524954),..Default::default()})]
    #[test]
    fn arbitrary_canonical_guid_replacements_agree_with_the_original_row_model(old in any::<[u8;16]>(),new in any::<[u8;16]>(),descending in any::<bool>()) {
        let bytes=guid_fixture::database(6,descending,&[Some(old),None,Some([0;16]),Some([255;16])],0);
        let candidate=run(&bytes,&[FixedScalarUpdate::new(0,value(Some(old)),value(Some(new)))]).unwrap();
        checked(candidate.candidate_bytes());
        let table=read_table(&mut Cursor::new(candidate.candidate_bytes()),CatalogScope::new(1),"Items",&mut budget(),&||false).unwrap();
        prop_assert_eq!(&table.rows()[0][1],&value(Some(new)));
    }
}

#[test]
fn ten_guid_keys_repack_full_segments_and_clear_individual_components() {
    use tuple_fixture::Cell::Guid as G;
    let rows = vec![
        vec![Some(G([1; 16])); 10],
        vec![Some(G([2; 16])); 10],
        vec![None; 10],
    ];
    let order = (0..10)
        .map(|column| (column, column % 2 != 0))
        .collect::<Vec<_>>();
    let bytes = tuple_fixture::database(6, &[15; 10], &order, &rows, 0);
    let first = [
        FixedScalarUpdate::new(0, value(Some([1; 16])), value(Some([3; 16]))),
        FixedScalarUpdate::new(2, Value::Null, value(Some([7; 16]))),
    ];
    let last = [FixedScalarUpdate::new(0, value(Some([1; 16])), Value::Null)];
    let candidate = capture(&bytes)
        .preview_ace16_batch_updates(
            &[
                FixedColumnUpdates::new("Items", "Id", &first),
                FixedColumnUpdates::new("Items", "K9", &last),
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    checked(candidate.candidate_bytes());
    assert_eq!(candidate.changed_rows(), 2);
    let table = read_table(
        &mut Cursor::new(candidate.candidate_bytes()),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows()[0][9], Value::Null);
    assert_eq!(table.rows()[2][0], value(Some([7; 16])));
    assert!(table.rows()[2][1..].iter().all(|v| *v == Value::Null));
}

#![cfg(feature = "ace16-write-preview")]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "foreign_updates/fixture.rs"]
mod foreign_fixture;
mod support;

use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
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
fn update(row: usize, old: i32, new: i32) -> FixedScalarUpdate {
    FixedScalarUpdate::new(row, Value::Int32(old), Value::Int32(new))
}
fn run(
    bytes: &[u8],
    groups: &[FixedColumnUpdates<'_>],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_cascading_updates(groups, &mut budget(), &|| false)
}
fn values(bytes: &[u8]) -> Vec<Vec<Value>> {
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
fn cascades_are_expanded_and_match_explicit_complete_batches() {
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = foreign_fixture::database(code, descending, true);
            let root = [update(0, 1, 9)];
            let groups = [FixedColumnUpdates::new("Items", "Id", &root)];
            let candidate = run(&bytes, &groups).unwrap();
            assert_eq!(candidate.changed_rows(), 3);
            let child = [update(1, 1, 9), update(2, 1, 9)];
            let explicit = capture(&bytes)
                .preview_ace16_batch_updates(
                    &[
                        FixedColumnUpdates::new("Items", "Id", &root),
                        FixedColumnUpdates::new("Items", "Score", &child),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.candidate_bytes(), explicit.candidate_bytes());
            assert_eq!(values(candidate.candidate_bytes())[2][1], Value::Int32(9));
        }
    }
}
#[test]
fn original_associations_survive_parent_key_permutations() {
    let bytes = foreign_fixture::database(6, false, true);
    let root = [update(0, 1, 2), update(1, 2, 1)];
    let candidate = run(&bytes, &[FixedColumnUpdates::new("Items", "Id", &root)]).unwrap();
    let rows = values(candidate.candidate_bytes());
    assert_eq!(rows[1], vec![Value::Int32(1), Value::Int32(2)]);
    assert_eq!(rows[2], vec![Value::Int32(3), Value::Int32(2)]);
    let reversed = [update(1, 2, 1), update(0, 1, 2)];
    assert_eq!(
        candidate.candidate_bytes(),
        run(&bytes, &[FixedColumnUpdates::new("Items", "Id", &reversed)])
            .unwrap()
            .candidate_bytes()
    );
    // Membership alone could accept unchanged foreign values; RESTRICT must not.
    let restricted = foreign_fixture::database(6, false, false);
    assert_eq!(
        run(
            &restricted,
            &[FixedColumnUpdates::new("Items", "Id", &root)]
        )
        .err(),
        Some(EditError::ReferentialAction)
    );
}
#[test]
fn compatible_explicit_demands_coalesce_but_conflicts_and_pinned_noops_fail() {
    let bytes = foreign_fixture::database(6, false, true);
    let root = [update(0, 1, 9)];
    for value in [1, 8, 9] {
        let child = [update(1, 1, value)];
        let groups = [
            FixedColumnUpdates::new("Items", "Id", &root),
            FixedColumnUpdates::new("Items", "Score", &child),
        ];
        let result = run(&bytes, &groups);
        if value == 9 {
            assert_eq!(result.unwrap().changed_rows(), 3);
        } else {
            assert_eq!(result.err(), Some(EditError::Conflict));
        }
        let reversed = [
            FixedColumnUpdates::new("Items", "Score", &child),
            FixedColumnUpdates::new("Items", "Id", &root),
        ];
        assert_eq!(run(&bytes, &reversed).is_ok(), value == 9);
    }
}
#[test]
fn duplicate_stale_empty_and_exact_noop_inputs_keep_the_batch_contract() {
    let bytes = foreign_fixture::database(6, false, true);
    assert_eq!(run(&bytes, &[]).unwrap().candidate_bytes(), bytes);
    let noop = [update(0, 1, 1)];
    let group = FixedColumnUpdates::new("Items", "Id", &noop);
    let candidate = run(&bytes, std::slice::from_ref(&group)).unwrap();
    assert_eq!(candidate.changed_rows(), 0);
    assert_eq!(candidate.candidate_bytes(), bytes);
    assert_eq!(
        run(
            &bytes,
            &[group, FixedColumnUpdates::new("Items", "Id", &noop)]
        )
        .err(),
        Some(EditError::Conflict)
    );
    let stale = [update(0, 2, 9)];
    assert_eq!(
        run(&bytes, &[FixedColumnUpdates::new("Items", "Id", &stale)]).err(),
        Some(EditError::Conflict)
    );
    let repeated = [update(0, 1, 9), update(0, 1, 9)];
    assert_eq!(
        run(&bytes, &[FixedColumnUpdates::new("Items", "Id", &repeated)]).err(),
        Some(EditError::Conflict)
    );
}
#[test]
fn cancellation_and_each_resource_exhaustion_return_no_candidate() {
    let bytes = foreign_fixture::database(6, false, true);
    let updates = [update(0, 1, 9)];
    let groups = [FixedColumnUpdates::new("Items", "Id", &updates)];
    for limit in [0, 50, 300] {
        let calls = Cell::new(0);
        assert!(
            capture(&bytes)
                .preview_ace16_cascading_updates(&groups, &mut budget(), &|| {
                    let n = calls.get();
                    calls.set(n + 1);
                    n >= limit
                })
                .is_err()
        );
    }
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&bytes)
                .preview_ace16_cascading_updates(&groups, &mut budget, &|| false)
                .is_err()
        );
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config { cases:128,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x43415343414445), ..Default::default() })]
    #[test]
    fn final_rows_follow_the_original_parent_model(keys in proptest::collection::vec(1_i32..100,3)) {
        proptest::prop_assume!(keys[0]!=keys[1] && keys[0]!=keys[2] && keys[1]!=keys[2]);
        let bytes=foreign_fixture::database(6,false,true);
        let request=(0..3).map(|row|update(row,row as i32+1,keys[row])).collect::<Vec<_>>();
        let candidate=run(&bytes,&[FixedColumnUpdates::new("Items","Id",&request),FixedColumnUpdates::new("Items","Score",&[])]).unwrap();
        let expected=vec![vec![Value::Int32(keys[0]),Value::Null],
            vec![Value::Int32(keys[1]),Value::Int32(keys[0])],
            vec![Value::Int32(keys[2]),Value::Int32(keys[0])]];
        proptest::prop_assert_eq!(values(candidate.candidate_bytes()),expected);
        let reverse=request.into_iter().rev().collect::<Vec<_>>();
        let other=run(&bytes,&[FixedColumnUpdates::new("Items","Id",&reverse)]).unwrap();
        proptest::prop_assert_eq!(candidate.candidate_bytes(),other.candidate_bytes());
    }
}

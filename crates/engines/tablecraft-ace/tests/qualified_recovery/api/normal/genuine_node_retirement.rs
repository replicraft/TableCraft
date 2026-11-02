//! Whole-image saves of independently qualified retained NODE populations.
#[path = "genuine_node_retirement/recovery.rs"]
mod recovery;
#[path = "genuine_node_retirement/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeDeleteIntent, NativeImage, RowDelete, inspect_table_index_counter, read_table,
    read_table_index,
};
use tablecraft_testkit::fixtures::access::genuine_node_retirement::{self as fixtures, Case};

fn cases() -> Vec<Case> {
    fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    let mut ordinals = case.recipe.selected().collect::<Vec<_>>();
    if reverse {
        ordinals.reverse();
    }
    ordinals
        .into_iter()
        .map(|ordinal| RowDelete::new(ordinal, fixtures::record(ordinal)))
        .collect()
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCND0001");
    raw.extend_from_slice(
        &u32::try_from(73 + 27 * case.recipe.selected().len())
            .unwrap()
            .to_le_bytes(),
    );
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for value in [5_u32, case.recipe.selected().len() as u32, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for request in requests(case, reverse) {
        raw.extend_from_slice(&(request.row() as u64).to_le_bytes());
        raw.extend_from_slice(&15_u32.to_le_bytes());
        raw.extend_from_slice(request.expected_bytes());
    }
    assert_eq!(raw.len(), 73 + 27 * case.recipe.selected().len());
    raw
}

fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, false), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires eight independent native/SDK-qualified NODE retirement source images"]
fn node_retirement_commits_both_original_orders_and_preserves_full_alias_history() {
    let mut commits = 0;
    for case in cases() {
        for reverse in [false, true] {
            let mut ordinals = case.recipe.selected().collect::<Vec<_>>();
            if reverse {
                ordinals.reverse();
            }
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes("Items", &ordinals, &mut budget(), &|| false)
                .unwrap();
            let mut captured = prepared.iter().map(RowDelete::row).collect::<Vec<_>>();
            captured.sort_unstable();
            assert_eq!(captured, case.recipe.selected().collect::<Vec<_>>());
            for request in prepared {
                assert_eq!(request.expected_bytes(), fixtures::record(request.row()));
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "Items",
                    requests(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), case.recipe.selected().len());
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case, reverse)
            );

            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let deleted = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap();
            assert_eq!(deleted, case.recipe.selected().len());
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );

            let sequential = read_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(sequential.rows(), fixtures::rows(Some(case.recipe), false));
            for (name, physical, history) in [
                ("PrimaryID", 0, 800),
                ("PrimaryAlias", 0, 800),
                ("GroupOrder", 1, 7),
                ("GroupAlias", 1, 7),
            ] {
                let rows = read_table_index(
                    &mut Cursor::new(target.bytes.get_ref()),
                    scope(),
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    rows.rows(),
                    fixtures::rows(Some(case.recipe), physical == 1)
                );
                let counter = inspect_table_index_counter(
                    &mut Cursor::new(target.bytes.get_ref()),
                    scope(),
                    "Items",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.physical_index(), physical);
                assert_eq!(counter.value(), history);
            }
            commits += 1;
        }
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_deletes("Items", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.deleted_rows(), 0);
        assert_eq!(empty.after_bytes(), case.before);
    }
    assert_eq!(commits, 12);
}

#[test]
#[ignore = "requires eight independent native/SDK-qualified NODE retirement source images"]
fn stale_raw_witnesses_and_missing_rows_cannot_publish_retirement_plans() {
    let mut refused = 0;
    for case in cases() {
        let ordinal = case.recipe.selected().start;
        let mut stale = fixtures::record(ordinal);
        stale[6] ^= 1;
        for request in [
            RowDelete::new(ordinal, stale),
            RowDelete::new(800, fixtures::record(0)),
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_row_deletes(
                    "Items",
                    vec![request],
                    &mut budget(),
                    &|| false
                ),
                Err(tablecraft_ace::RecoverableEditError::Edit(
                    tablecraft_ace::EditError::Conflict
                ))
            ));
            refused += 1;
        }
    }
    assert_eq!(refused, 12);
}

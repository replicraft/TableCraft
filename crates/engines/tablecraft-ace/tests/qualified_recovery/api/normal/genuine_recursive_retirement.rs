//! Whole-image saves of independently qualified retained NODE populations.
#[path = "genuine_recursive_retirement/recovery.rs"]
mod recovery;
#[path = "genuine_recursive_retirement/resources.rs"]
mod resources;

use super::{Storage, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeDeleteIntent, NativeImage, RowDelete, inspect_table_index_counter, read_table,
    read_table_index,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_testkit::access::genuine_recursive_retirement::{self as fixtures, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

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
        &u32::try_from(81 + 25 * case.recipe.selected().len())
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
    for value in [13_u32, case.recipe.selected().len() as u32, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"BinaryStorage");
    for request in requests(case, reverse) {
        raw.extend_from_slice(&(request.row() as u64).to_le_bytes());
        raw.extend_from_slice(&13_u32.to_le_bytes());
        raw.extend_from_slice(request.expected_bytes());
    }
    assert_eq!(raw.len(), 81 + 25 * case.recipe.selected().len());
    raw
}

fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, false), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires six independent native/SDK-qualified recursive retirement source images"]
fn node_retirement_commits_both_original_orders_and_preserves_full_alias_history() {
    let mut commits = 0;
    for case in cases() {
        for reverse in [false, true] {
            let mut ordinals = case.recipe.selected().collect::<Vec<_>>();
            if reverse {
                ordinals.reverse();
            }
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes("BinaryStorage", &ordinals, &mut budget(), &|| false)
                .unwrap();
            let mut captured = prepared.iter().map(RowDelete::row).collect::<Vec<_>>();
            captured.sort_unstable();
            assert_eq!(captured, case.recipe.selected().collect::<Vec<_>>());
            for request in prepared {
                assert_eq!(request.expected_bytes(), fixtures::record(request.row()));
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "BinaryStorage",
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

            let original_wire = wire(&case, reverse);
            let decoded =
                NativeDeleteIntent::decode(&original_wire, &mut budget(), &|| false).unwrap();
            assert_eq!(
                decoded.encode(&mut budget(), &|| false).unwrap(),
                original_wire
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
                "BinaryStorage",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(sequential.rows(), fixtures::rows(Some(case.recipe)));
            let rows = read_table_index(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "BinaryStorage",
                "PrimaryID",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), fixtures::rows(Some(case.recipe)));
            let counter = inspect_table_index_counter(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "BinaryStorage",
                "PrimaryID",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), 149184);
            commits += 1;
        }
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_deletes("BinaryStorage", vec![], &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(empty.deleted_rows(), 0);
        assert!(empty.intent().deletes().is_empty());
        assert_eq!(empty.after_bytes(), case.before);
    }
    assert_eq!(commits, 8);
}

#[test]
#[ignore = "requires six independent native/SDK-qualified recursive retirement source images"]
fn stale_raw_witnesses_and_missing_rows_cannot_publish_retirement_plans() {
    let mut refused = 0;
    for case in cases() {
        let ordinal = case.recipe.selected().start;
        let mut stale = fixtures::record(ordinal);
        stale[6] ^= 1;
        for request in [
            RowDelete::new(ordinal, stale),
            RowDelete::new(963, fixtures::record(0)),
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_row_deletes(
                    "BinaryStorage",
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
    assert_eq!(refused, 8);
}

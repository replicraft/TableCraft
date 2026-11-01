//! Whole-image saves of independently qualified retained NODE populations.
#[path = "genuine_retained_node_delete/recovery.rs"]
mod recovery;
#[path = "genuine_retained_node_delete/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeDeleteIntent, NativeImage, RowDelete, inspect_table_index_counter, read_table,
    read_table_index,
};
use tablecraft_testkit::access::genuine_retained_node_delete::{self as fixtures, Case};

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

fn requests(reverse: bool) -> Vec<RowDelete> {
    let ordinals = if reverse { [353, 301] } else { [301, 353] };
    ordinals
        .into_iter()
        .map(|ordinal| RowDelete::new(ordinal, fixtures::record(ordinal)))
        .collect()
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCND0001");
    raw.extend_from_slice(&127_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for value in [5_u32, 2, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for request in requests(reverse) {
        raw.extend_from_slice(&(request.row() as u64).to_le_bytes());
        raw.extend_from_slice(&15_u32.to_le_bytes());
        raw.extend_from_slice(request.expected_bytes());
    }
    assert_eq!(raw.len(), 127);
    raw
}

fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, false), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires four independent native/SDK-qualified retained NODE source images"]
fn retained_node_deletion_commits_both_orders_and_preserves_full_fence_identity() {
    let mut commits = 0;
    for case in cases() {
        for reverse in [false, true] {
            let ordinals = if reverse { [353, 301] } else { [301, 353] };
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes("Items", &ordinals, &mut budget(), &|| false)
                .unwrap();
            let mut captured = prepared.iter().map(RowDelete::row).collect::<Vec<_>>();
            captured.sort_unstable();
            assert_eq!(captured, [301, 353]);
            for request in prepared {
                assert_eq!(request.expected_bytes(), fixtures::record(request.row()));
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "Items",
                    requests(reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), 2);
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
            assert_eq!(deleted, 2);
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
            assert_eq!(sequential.rows(), fixtures::rows(true, false));
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
                assert_eq!(rows.rows(), fixtures::rows(true, physical == 1));
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
    assert_eq!(commits, 4);
}

#[path = "genuine_retained_node_delete/scope_controls.rs"]
mod scope_controls;

//! Real TCNI5 commits against independently qualified mixed whole images.
#[path = "external_lval_mixed/recovery.rs"]
mod recovery;
#[path = "external_lval_mixed/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeImage, NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError,
    read_table,
};
use tablecraft_testkit::fixtures::access::genuine_external_lval_mixed::{self as fixture, Case};

fn from_original(
    groups: Vec<tablecraft_testkit::fixtures::access::genuine_external_lval_update::Group>,
) -> Vec<tablecraft_ace::NativeFittedIntentGroup> {
    fixture::map_groups(
        groups,
        tablecraft_ace::SlotScalarUpdate::new,
        |column, updates| {
            tablecraft_ace::NativeFittedIntentGroup::new(
                column,
                tablecraft_ace::NativeFittedUpdates::Variable(updates),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        },
    )
}

fn intent_groups(case: &Case, reverse: bool) -> Vec<tablecraft_ace::NativeFittedIntentGroup> {
    from_original(fixture::request_groups(case, reverse))
}

fn cases() -> Vec<Case> {
    fixture::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
    .into_iter()
    .filter(|case| case.recipe != "original")
    .collect()
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    fixture::intent_wire(case, reverse, digest.finalize().into())
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_all_original_group_orders_commit_literal_images_and_noops() {
    let cases = cases();
    assert_eq!(cases.len(), 16);
    // Literal wires precede every matching planner invocation.
    let wires = cases
        .iter()
        .map(|case| [wire(case, false), wire(case, true)])
        .collect::<Vec<_>>();
    let mut commits = 0;
    let mut noops = 0;
    let mut empty_sources = std::collections::BTreeSet::new();
    let mut targetless_sources = std::collections::BTreeSet::new();
    for (case, wires) in cases.iter().zip(&wires) {
        for reverse in [false, true] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    intent_groups(case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), fixture::changed_rows(case));
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFitted
            );
            let original = &wires[usize::from(reverse)];
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                *original
            );
            let decoded = NativeUpdateIntent::decode(original, &mut budget(), &|| false).unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), *original);
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                fixture::changed_rows(case)
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            assert_eq!(
                read_table(
                    &mut Cursor::new(target.bytes.get_ref()),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                case.rows.as_slice()
            );
            commits += 1;

            let mut original_groups = fixture::request_groups(case, reverse);
            for group in &mut original_groups {
                for cell in &mut group.updates {
                    cell.expected = cell.replacement.clone();
                }
            }
            let owned = from_original(original_groups);
            let noop = image(&case.after)
                .plan_recoverable_ordinary_fitted_updates("Items", owned, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(noop.changed_rows(), 0);
            assert_eq!(noop.after_bytes(), case.after);
            noops += 1;
            let mut unchanged = vec![(noop, &case.after)];
            let source_key = Sha256::digest(&case.before).to_vec();
            if targetless_sources.insert(source_key.clone()) {
                assert!(matches!(
                    image(&case.before).plan_recoverable_ordinary_fitted_updates(
                        "Items",
                        Vec::new(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(RecoverableEditError::Intent(
                        tablecraft_ace::NativeIntentError::Inspect(
                            tablecraft_ace::InspectError::StructureLimit
                        )
                    ))
                ));
            }
            if empty_sources.insert((source_key, reverse)) {
                use tablecraft_testkit::fixtures::access::genuine_external_lval_update::Group;
                let mut columns = vec![
                    Group {
                        column: "Blob",
                        updates: Vec::new(),
                    },
                    Group {
                        column: "Memo",
                        updates: Vec::new(),
                    },
                ];
                if reverse {
                    columns.reverse();
                }
                let empty = image(&case.before)
                    .plan_recoverable_ordinary_fitted_updates(
                        "Items",
                        from_original(columns),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(empty.changed_rows(), 0);
                assert_eq!(empty.after_bytes(), case.before);
                unchanged.push((empty, &case.before));
            }
            for (unchanged, expected) in unchanged {
                let before = unchanged.before_bytes().to_vec();
                let mut target = Storage::new(before.clone());
                let mut journal = Storage::new(Vec::new());
                assert_eq!(
                    unchanged
                        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap()
                        .commit(&mut budget(), &|| false)
                        .unwrap(),
                    0
                );
                assert_eq!(target.bytes.get_ref(), expected);
                assert_eq!(
                    journal.bytes.get_ref(),
                    &journals::journal(&before, expected, Marker::Committed)
                );
            }
        }
    }
    assert_eq!(
        (
            commits,
            noops,
            empty_sources.len(),
            targetless_sources.len()
        ),
        (32, 32, 8, 4)
    );
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_stale_retiring_and_retained_carriers_refuse_atomically() {
    let mut refused = 0;
    for case in cases() {
        for reverse in [false, true] {
            for stale in 0..2 {
                let mut original = fixture::request_groups(&case, reverse);
                original[stale].updates[0].expected = tablecraft_core::values::Value::Null;
                let owned = from_original(original);
                assert!(matches!(
                    image(&case.before).plan_recoverable_ordinary_fitted_updates(
                        "Items",
                        owned,
                        &mut budget(),
                        &|| false,
                    ),
                    Err(RecoverableEditError::Edit(EditError::Conflict))
                ));
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 64);
}

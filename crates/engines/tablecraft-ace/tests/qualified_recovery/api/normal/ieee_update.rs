//! Exact ordinary IEEE UPDATE uses original TCNI8 and checked V1 recovery.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, qualify_native_update_recovery,
    read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Single, Value},
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::genuine_ieee_update::{self as source, Case},
    support::ace::resources::assert_public_boundary,
};

fn all() -> Vec<Case> {
    source::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<NativeFixedIntentGroup>, NativeIntentError> {
    case.groups()
        .into_iter()
        .map(|group| {
            NativeFixedIntentGroup::new_ordinary_ieee(
                group.column,
                vec![FixedScalarUpdate::new(2, group.expected, group.replacement)],
                work,
                cancelled,
            )
        })
        .collect()
}

fn wire(case: &Case) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize().into())
}

#[test]
#[ignore = "requires qualified existing IEEE source pairs"]
fn ieee_original_intent_commits_literal_images_and_reopens_exact_bits() {
    let mut commits = 0;
    for case in all() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(
                "Items",
                groups(&case, &mut budget(), &|| false).unwrap(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryIeeeFixed
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        let decoded = NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        let mut target = Storage::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            log.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, journals::Marker::Committed)
        );
        assert_eq!(
            read_table(
                &mut image(&case.after).reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.rows(true)
        );
        commits += 1;
    }
    assert_eq!(commits, 4);
}

#[test]
#[ignore = "requires qualified existing IEEE sources"]
fn ieee_witnesses_are_bit_exact_while_noop_and_named_empty_keep_complete_bytes() {
    let mut controls = [0; 2];
    for case in all() {
        let original = case.groups();
        for (column, wrong) in [
            (
                "SingleValue",
                Value::Single(Single::from_bits(if case.subnormal_to_special {
                    2
                } else {
                    0
                })),
            ),
            (
                "DoubleValue",
                Value::Double(Double::from_bits(if case.subnormal_to_special {
                    2
                } else {
                    0
                })),
            ),
        ] {
            let replacement = original
                .iter()
                .find(|group| group.column == column)
                .unwrap()
                .replacement
                .clone();
            let group = NativeFixedIntentGroup::new_ordinary_ieee(
                column,
                vec![FixedScalarUpdate::new(2, wrong, replacement)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Items",
                        vec![group],
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(RecoverableEditError::Edit(EditError::Conflict))
            );
            controls[0] += 1;
        }
        if case.subnormal_to_special {
            for (column, wrong, exact) in [
                (
                    "SingleValue",
                    Value::Single(Single::from_bits(0x7f81_2344)),
                    Value::Single(Single::from_bits(0x7f81_2345)),
                ),
                (
                    "DoubleValue",
                    Value::Double(Double::from_bits(0xfff8_1234_5678_9abd)),
                    Value::Double(Double::from_bits(0xfff8_1234_5678_9abc)),
                ),
            ] {
                let group = NativeFixedIntentGroup::new_ordinary_ieee(
                    column,
                    vec![FixedScalarUpdate::new(2, wrong, exact.clone())],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    image(&case.after)
                        .plan_recoverable_ordinary_fixed_updates(
                            "Items",
                            vec![group],
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(RecoverableEditError::Edit(EditError::Conflict))
                );
                controls[0] += 1;
                let group = NativeFixedIntentGroup::new_ordinary_ieee(
                    column,
                    vec![FixedScalarUpdate::new(2, exact.clone(), exact)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let plan = image(&case.after)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Items",
                        vec![group],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.changed_rows(), 0);
                controls[1] += 1;
            }
        }
        for column in ["SingleValue", "DoubleValue"] {
            let exact = original
                .iter()
                .find(|group| group.column == column)
                .unwrap()
                .expected
                .clone();
            for empty in [false, true] {
                let updates = if empty {
                    vec![]
                } else {
                    vec![FixedScalarUpdate::new(2, exact.clone(), exact.clone())]
                };
                let group = NativeFixedIntentGroup::new_ordinary_ieee(
                    column,
                    updates,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let plan = image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Items",
                        vec![group],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), case.before);
                assert_eq!(plan.changed_rows(), 0);
                assert_eq!(
                    plan.intent().profile(),
                    NativeUpdateProfile::Ace16OrdinaryIeeeFixed
                );
                controls[1] += 1;
            }
        }
    }
    assert_eq!(controls, [12, 20]);
}

#[test]
#[ignore = "requires qualified existing IEEE sources"]
fn ieee_public_pools_and_cancellation_cover_capture_intent_and_independent_proof() {
    for case in all() {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let groups = groups(&case, work, cancelled).map_err(|error| match error {
                    NativeIntentError::Inspect(error) => error,
                    other => panic!("unexpected request error: {other:?}"),
                })?;
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates("Items", groups, work, cancelled)
                    .map_err(|error| match error {
                        RecoverableEditError::Edit(EditError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Request(
                            EditError::Inspect(error),
                        )) => error,
                        other => panic!("unexpected public IEEE error: {other:?}"),
                    })
            },
            |plan| {
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.changed_rows(), 1);
            },
            |plan| {
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.changed_rows(), 1);
            },
        );
    }
}

#[test]
#[ignore = "requires qualified existing IEEE source pairs"]
fn ieee_checked_recovery_replays_original_bits_and_refuses_committed_mixtures() {
    let mut contexts = 0;
    for case in all() {
        let mut mixed = case.before.clone();
        let single = 8 * 4096 + 3907 + 25;
        mixed[single..single + 4].copy_from_slice(&case.after[single..single + 4]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for (marker, original, mismatch) in [
            (journals::Marker::Pending, case.before.clone(), false),
            (journals::Marker::Partial, mixed.clone(), false),
            (journals::Marker::Committed, case.after.clone(), false),
            (journals::Marker::Committed, mixed.clone(), true),
        ] {
            let committed = matches!(marker, journals::Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let mut target = Storage::new(original.clone());
            let mut log = Storage::new(journals::journal(&case.before, &case.after, marker));
            let saved = log.bytes.get_ref().clone();
            target.fail_prefix = mismatch.then_some(0);
            target.fail_sync = mismatch;
            log.fail_sync = mismatch;
            let intent =
                NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            let qualified =
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .unwrap();
            let result = qualified.recover(&mut target, &mut budget(), &|| false);
            if mismatch {
                assert!(matches!(result, Err(JournalError::TargetMismatch)));
                assert_eq!(target.bytes.get_ref(), &original);
                assert_eq!(target.writes, 0);
                assert!(target.fail_sync && log.fail_sync);
            } else {
                assert_eq!(result.unwrap(), outcome);
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &case.after } else { &case.before }
                );
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let intent =
                    NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
                let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .confirm_completed(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    outcome
                );
                assert_eq!(target.writes, writes);
            }
            assert_eq!(log.writes, 0);
            assert_eq!(log.bytes.get_ref(), &saved);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 16);
}

#[test]
#[ignore = "requires six pinned IEEE input images; decoder makes no native plan"]
fn ieee_literal_dialect_rejects_padding_truncation_and_legacy_carrier_expansion() {
    for case in all() {
        let wire = wire(&case);
        let first_cell = 72 + 5 + 12 + "SingleValue".len() + 8;
        assert_eq!(wire[first_cell], 6);
        for offset in [14, first_cell, first_cell + 5] {
            let mut bad = wire.clone();
            bad[offset] = 0xff;
            assert!(NativeUpdateIntent::decode(&bad, &mut budget(), &|| false).is_err());
        }
        for (magic, profile) in [(b"TCNI0003", 3_u8), (b"TCNI0004", 4)] {
            let mut legacy = wire.clone();
            legacy[..8].copy_from_slice(magic);
            legacy[12] = profile;
            assert!(NativeUpdateIntent::decode(&legacy, &mut budget(), &|| false).is_err());
        }
        let mut mismatch = wire.clone();
        mismatch[12] = 4;
        assert!(NativeUpdateIntent::decode(&mismatch, &mut budget(), &|| false).is_err());
        for end in 0..wire.len() {
            assert!(NativeUpdateIntent::decode(&wire[..end], &mut budget(), &|| false).is_err());
        }
        let mut extra = wire.clone();
        extra.push(0);
        assert!(NativeUpdateIntent::decode(&extra, &mut budget(), &|| false).is_err());
    }
}

#[test]
#[ignore = "requires qualified exact IEEE originals; legacy named-empty requests carry no float tags"]
fn legacy_named_empty_ieee_columns_decode_but_cold_qualification_refuses_before_effects() {
    use tablecraft_ace::{NativeRecoveryQualificationError, WriteBlocker};
    use tablecraft_testkit::fixtures::access::original_fixed_wire::single_column_original_wire;

    let mut contexts = 0;
    for case in all().into_iter().filter(|case| !case.subnormal_to_special) {
        let mut digest = Sha256::new();
        digest.update(b"TCNI0001:BEFORE\0");
        digest.update((case.before.len() as u64).to_le_bytes());
        digest.update(&case.before);
        let binding: [u8; 32] = digest.finalize().into();
        for column in ["SingleValue", "DoubleValue"] {
            let literal =
                single_column_original_wire("Items", column, &[], case.before.len(), binding);
            let intent = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
            assert_eq!(intent.profile(), NativeUpdateProfile::Ace16OrdinaryFixed);
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal);

            // Equal complete images are a valid generic V1 journal; its checked
            // framing does not authorize an old dialect to name a new target.
            let mut log = Storage::new(journals::journal(
                &case.before,
                &case.before,
                journals::Marker::Pending,
            ));
            let saved = log.bytes.get_ref().clone();
            log.fail_prefix = Some(0);
            log.fail_sync = true;
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::Edit(
                    EditError::Unsupported(WriteBlocker::Column)
                )),
            ));
            assert_eq!(log.writes, 0);
            assert_eq!(log.bytes.get_ref(), &saved);
            assert_eq!(log.fail_prefix, Some(0));
            assert!(log.fail_sync);
            // No target is passed to a failed qualifier, so recovery/confirmation
            // capabilities and their target write authority were never issued.
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
}

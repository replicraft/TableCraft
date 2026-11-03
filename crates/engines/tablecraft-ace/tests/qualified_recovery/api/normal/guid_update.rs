//! Unindexed canonical GUID intents and mixed-endian native payloads stay distinct.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, WriteBlocker, qualify_native_update_recovery, read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Single, Value},
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::{
        genuine_guid_update::{self as source, Case, Recipe},
        original_fixed_wire::{self, Dialect, Group},
    },
    support::ace::resources::assert_public_boundary,
};

fn all() -> Vec<Case> {
    source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn binding(before: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    digest.finalize().into()
}

fn group(
    row: usize,
    before: Value,
    after: Value,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeFixedIntentGroup, NativeIntentError> {
    NativeFixedIntentGroup::new_ordinary_guid(
        "GuidValue",
        vec![FixedScalarUpdate::new(row, before, after)],
        work,
        cancelled,
    )
}

#[test]
#[ignore = "requires qualified unindexed GUID originals and span laws"]
fn guid10_original_canonical_wire_commits_exact_native_payload_and_mask_images() {
    let mut commits = 0;
    for case in all() {
        let (row, before, after) = case.request();
        let group = group(row, before, after, &mut budget(), &|| false).unwrap();
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", vec![group], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryGuidFixed
        );
        let literal = case.wire(binding(&case.before));
        assert_eq!(&literal[..8], b"TCNI0010");
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );
        let decoded = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal);
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
    assert_eq!(commits, 6);
}

#[test]
#[ignore = "requires admitted originals and NULL state; original canonical GUID remains present"]
fn guid10_original_null_empty_and_stale_witnesses_keep_exact_type_and_presence() {
    let mut counts = [0; 3];
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Present)
    {
        let exact = Value::Guid(Guid::from_bytes(source::OLD_CANONICAL));
        for row in [0, 2] {
            let value = if row == 0 {
                Value::Guid(Guid::from_bytes([
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
                ]))
            } else {
                exact.clone()
            };
            let group = group(row, value.clone(), value, &mut budget(), &|| false).unwrap();
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
                NativeUpdateProfile::Ace16OrdinaryGuidFixed
            );
            counts[0] += 1;
        }
        let empty =
            NativeFixedIntentGroup::new_ordinary_guid("GuidValue", vec![], &mut budget(), &|| {
                false
            })
            .unwrap();
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", vec![empty], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.before);
        assert_eq!(plan.changed_rows(), 0);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryGuidFixed
        );
        counts[0] += 1;
        for byte in [0, 15] {
            let mut stale = source::OLD_CANONICAL;
            stale[byte] ^= 1;
            let group = group(
                2,
                Value::Guid(Guid::from_bytes(stale)),
                exact.clone(),
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
            counts[1] += 1;
        }
        for (before, after) in [
            (Value::Int64(0), exact.clone()),
            (exact.clone(), Value::Int64(0)),
        ] {
            let group = group(2, before, after, &mut budget(), &|| false).unwrap();
            assert_eq!(
                image(&case.before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Items",
                        vec![group],
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            );
            counts[2] += 1;
        }
    }
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullRestore)
    {
        let noop = group(2, Value::Null, Value::Null, &mut budget(), &|| false).unwrap();
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", vec![noop], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), case.before);
        assert_eq!(plan.changed_rows(), 0);
        counts[0] += 1;
        let exact = Value::Guid(Guid::from_bytes(source::OLD_CANONICAL));
        let stale = group(2, exact.clone(), exact, &mut budget(), &|| false).unwrap();
        assert_eq!(
            image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    vec![stale],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(RecoverableEditError::Edit(EditError::Conflict))
        );
        counts[1] += 1;
    }
    assert_eq!(counts, [8, 6, 4]);
}

#[test]
#[ignore = "requires exact GUID originals; old4/8/9 named-empty dialects remain closed"]
fn old_9byte_dialects_cannot_cold_qualify_an_empty_guid_target() {
    let mut contexts = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Present)
    {
        for dialect in [Dialect::Ordinary, Dialect::Ieee, Dialect::Scalar] {
            let literal = original_fixed_wire::original_wire(
                dialect,
                "Items",
                &[Group {
                    column: "GuidValue",
                    updates: &[],
                }],
                case.before.len(),
                binding(&case.before),
            );
            let intent = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal);
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
                ))
            ));
            assert_eq!(log.writes, 0);
            assert_eq!(log.bytes.get_ref(), &saved);
            assert!(log.fail_sync);
            assert_eq!(log.fail_prefix, Some(0));
            contexts += 1;
        }
    }
    assert_eq!(contexts, 6);
}

#[test]
#[ignore = "requires qualified GUID spans; full public17byte request and proof costs"]
fn guid10_public_pools_and_cancellation_cover_complete_preparation() {
    for case in all() {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let (row, before, after) = case.request();
                let group =
                    group(row, before, after, work, cancelled).map_err(|error| match error {
                        NativeIntentError::Inspect(error) => error,
                        other => panic!("unexpected literal request: {other:?}"),
                    })?;
                let image =
                    NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)?;
                image
                    .plan_recoverable_ordinary_fixed_updates("Items", vec![group], work, cancelled)
                    .map_err(|error| match error {
                        RecoverableEditError::Edit(EditError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
                        | RecoverableEditError::Intent(NativeIntentError::Request(
                            EditError::Inspect(error),
                        )) => error,
                        other => panic!("unexpected public GUID plan: {other:?}"),
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
#[ignore = "requires exact GUID originals; representative canonical16 cold recovery"]
fn guid10_checked_recovery_replays_canonical_intent_and_refuses_native_mixtures() {
    let mut contexts = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Present)
    {
        let mut mixed = case.before.clone();
        let prefix = 8 * 4096 + 3907 + 45;
        mixed[prefix..prefix + 4].copy_from_slice(&case.after[prefix..prefix + 4]);
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
            let intent = NativeUpdateIntent::decode(
                &case.wire(binding(&case.before)),
                &mut budget(),
                &|| false,
            )
            .unwrap();
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
                let intent = NativeUpdateIntent::decode(
                    &case.wire(binding(&case.before)),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
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
    assert_eq!(contexts, 8);
}

fn assert_mixed_guid_profile(case: &Case, sibling: &str, empty: bool, reverse: bool) {
    let (row, before, after) = case.request();
    let guid_updates = [(row, before.clone(), after.clone())];
    let sibling_value = match sibling {
        "ByteValue" => Value::Byte(255),
        "SingleValue" => Value::Single(Single::from_bits(0x8000_0000)),
        _ => unreachable!(),
    };
    let sibling_updates = if empty {
        Vec::new()
    } else {
        vec![(2, sibling_value.clone(), sibling_value)]
    };
    let guid = group(row, before, after, &mut budget(), &|| false).unwrap();
    let updates = sibling_updates
        .iter()
        .map(|(row, before, after)| FixedScalarUpdate::new(*row, before.clone(), after.clone()))
        .collect();
    let sibling_group = if sibling == "ByteValue" {
        NativeFixedIntentGroup::new_ordinary_scalar(sibling, updates, &mut budget(), &|| false)
    } else {
        NativeFixedIntentGroup::new_ordinary_ieee(sibling, updates, &mut budget(), &|| false)
    }
    .unwrap();
    let mut native = vec![guid, sibling_group];
    let mut literal = vec![
        Group {
            column: "GuidValue",
            updates: &guid_updates,
        },
        Group {
            column: sibling,
            updates: &sibling_updates,
        },
    ];
    if reverse {
        native.reverse();
        literal.reverse();
    }
    let plan = image(&case.before)
        .plan_recoverable_ordinary_fixed_updates("Items", native, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16OrdinaryGuidFixed
    );
    assert_eq!(plan.changed_rows(), 1);
    assert_eq!(plan.after_bytes(), case.after);
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        original_fixed_wire::original_wire(
            Dialect::Guid,
            "Items",
            &literal,
            case.before.len(),
            binding(&case.before)
        )
    );
}

#[test]
#[ignore = "requires original GUID pair; metadata GUID dominates scalar and IEEE in both orders"]
fn guid10_metadata_join_keeps_empty_and_noop_siblings_order_independent() {
    let mut contexts = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Present)
    {
        for sibling in ["ByteValue", "SingleValue"] {
            for empty in [false, true] {
                for reverse in [false, true] {
                    assert_mixed_guid_profile(&case, sibling, empty, reverse);
                    contexts += 1;
                }
            }
        }
    }
    assert_eq!(contexts, 16);
}

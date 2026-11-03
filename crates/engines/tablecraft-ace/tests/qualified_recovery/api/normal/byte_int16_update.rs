//! Exact scalar9 targets retain original width, NULL payload and cold policy.
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
    values::{Single, Value},
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::{
        genuine_byte_int16_update::{self as source, Case, Recipe},
        genuine_ieee_update,
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

fn groups(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<NativeFixedIntentGroup>, NativeIntentError> {
    case.requests()
        .into_iter()
        .map(|(column, before, after)| {
            NativeFixedIntentGroup::new_ordinary_scalar(
                column,
                vec![FixedScalarUpdate::new(2, before, after)],
                work,
                cancelled,
            )
        })
        .collect()
}

fn binding(before: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    digest.finalize().into()
}

#[test]
#[ignore = "requires qualified existing exact originals and literal Byte/Int16 spans"]
fn scalar9_commits_endpoint_and_null_spans_with_independent_original_wire() {
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
            NativeUpdateProfile::Ace16OrdinaryScalarFixed
        );
        let literal = case.wire(binding(&case.before));
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );
        let intent = NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), literal);
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
#[ignore = "requires exact originals; every named target is requalified even without changes"]
fn scalar9_exact_carriers_noops_and_empty_targets_keep_original_bytes() {
    let mut counts = [0; 4];
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Endpoints)
    {
        for (column, original) in [
            ("ByteValue", Value::Byte(255)),
            ("Int16Value", Value::Int16(-32_768)),
        ] {
            for empty in [false, true] {
                let updates = if empty {
                    vec![]
                } else {
                    vec![FixedScalarUpdate::new(
                        2,
                        original.clone(),
                        original.clone(),
                    )]
                };
                let group = NativeFixedIntentGroup::new_ordinary_scalar(
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
                    NativeUpdateProfile::Ace16OrdinaryScalarFixed
                );
                counts[0] += 1;
            }
            // Correct carrier and presence, but stale present payload. If this
            // witness were ignored the replacement would otherwise be a no-op.
            let stale = if column == "ByteValue" {
                Value::Byte(254)
            } else {
                Value::Int16(-32_767)
            };
            let group = NativeFixedIntentGroup::new_ordinary_scalar(
                column,
                vec![FixedScalarUpdate::new(2, stale, original.clone())],
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
                        &|| false,
                    )
                    .err(),
                Some(RecoverableEditError::Edit(EditError::Conflict))
            );
            counts[3] += 1;

            let wrong = Value::Int32(if column == "ByteValue" { 255 } else { -32_768 });
            let group = NativeFixedIntentGroup::new_ordinary_scalar(
                column,
                vec![FixedScalarUpdate::new(2, wrong, original)],
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
                Some(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::Column
                )))
            );
            counts[1] += 1;
        }
    }
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullRestore)
    {
        for (column, inactive) in [
            ("ByteValue", Value::Byte(255)),
            ("Int16Value", Value::Int16(-32_768)),
        ] {
            let group = NativeFixedIntentGroup::new_ordinary_scalar(
                column,
                vec![FixedScalarUpdate::new(2, inactive.clone(), inactive)],
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
            counts[2] += 1;
        }
    }
    assert_eq!(counts, [8, 4, 4, 4]);
}

#[test]
#[ignore = "requires qualified exact originals; old dialects must not name new fields"]
fn old4_and8_named_empty_small_fields_decode_but_cold_qualification_refuses() {
    let mut contexts = 0;
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Endpoints)
    {
        for column in ["ByteValue", "Int16Value"] {
            for dialect in [Dialect::Ordinary, Dialect::Ieee] {
                let literal = original_fixed_wire::original_wire(
                    dialect,
                    "Items",
                    &[Group {
                        column,
                        updates: &[],
                    }],
                    case.before.len(),
                    binding(&case.before),
                );
                let intent =
                    NativeUpdateIntent::decode(&literal, &mut budget(), &|| false).unwrap();
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
                    qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::Edit(
                        EditError::Unsupported(WriteBlocker::Column)
                    ))
                ));
                assert_eq!(log.writes, 0);
                assert_eq!(log.bytes.get_ref(), &saved);
                assert_eq!(log.fail_prefix, Some(0));
                assert!(log.fail_sync);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 8);
}

#[test]
#[ignore = "requires qualified exact originals; independent scalar9 cell padding"]
fn scalar9_width1_and2_padding_stays_canonical_and_old_decoders_stay_closed() {
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Endpoints)
    {
        let wire = case.wire(binding(&case.before));
        let byte_cell = 72 + 5 + 12 + "ByteValue".len() + 8;
        let integer_cell = 72 + 5 + 12 + "ByteValue".len() + 26 + 12 + "Int16Value".len() + 8;
        assert_eq!(wire[byte_cell], 8);
        assert_eq!(wire[integer_cell], 9);
        for offset in [byte_cell + 2, integer_cell + 3] {
            let mut bad = wire.clone();
            bad[offset] = 1;
            assert!(NativeUpdateIntent::decode(&bad, &mut budget(), &|| false).is_err());
        }
        for (magic, profile) in [(b"TCNI0004", 4), (b"TCNI0008", 8)] {
            let mut bad = wire.clone();
            bad[..8].copy_from_slice(magic);
            bad[12] = profile;
            assert!(NativeUpdateIntent::decode(&bad, &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
#[ignore = "requires exact originals and qualified endpoint/null masks"]
fn scalar9_public_pools_and_cancellation_cover_complete_preparation() {
    for case in all() {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let groups = groups(&case, work, cancelled).map_err(|error| match error {
                    NativeIntentError::Inspect(error) => error,
                    other => panic!("unexpected literal request: {other:?}"),
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
                        other => panic!("unexpected public scalar9 plan: {other:?}"),
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
#[ignore = "requires endpoint and NULL span sources; mask-only NULL has no false partial mixture"]
fn scalar9_checked_recovery_distinguishes_payload_mixtures_from_single_mask_changes() {
    let mut counts = [0; 2];
    for case in all() {
        let mut states = vec![
            (journals::Marker::Pending, case.before.clone(), false),
            (journals::Marker::Committed, case.after.clone(), false),
        ];
        if case.recipe == Recipe::Endpoints {
            let mut mixed = case.before.clone();
            let byte = 8 * 4096 + 3907 + 2;
            mixed[byte] = case.after[byte];
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            states.push((journals::Marker::Partial, mixed.clone(), false));
            states.push((journals::Marker::Committed, mixed, true));
        } else {
            // NULL changes only one shared mask byte, so an extra byte-mixture
            // state would duplicate BEFORE or AFTER rather than add coverage.
            states.push((journals::Marker::Committed, case.before.clone(), true));
        }
        for (marker, original, mismatch) in states {
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
                counts[1] += 1;
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
            counts[0] += 1;
        }
    }
    assert_eq!(counts, [20, 6]);
}

type JoinedRequest<'a> = (&'a str, Vec<(usize, Value, Value)>);

fn assert_scalar_join(before: &[u8], after: &[u8], requests: &[JoinedRequest<'_>]) {
    let groups = requests
        .iter()
        .map(|(column, updates)| {
            NativeFixedIntentGroup::new_ordinary_scalar(
                column,
                updates
                    .iter()
                    .map(|(row, before, after)| {
                        FixedScalarUpdate::new(*row, before.clone(), after.clone())
                    })
                    .collect(),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        })
        .collect();
    let plan = image(before)
        .plan_recoverable_ordinary_fixed_updates("Items", groups, &mut budget(), &|| false)
        .unwrap();
    let literal: Vec<_> = requests
        .iter()
        .map(|(column, updates)| Group { column, updates })
        .collect();
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16OrdinaryScalarFixed
    );
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        original_fixed_wire::original_wire(
            Dialect::Scalar,
            "Items",
            &literal,
            before.len(),
            binding(before)
        )
    );
    assert_eq!(plan.after_bytes(), after);
    assert_eq!(plan.changed_rows(), 1);
}

#[test]
#[ignore = "requires qualified IEEE pair and Byte endpoint originals; no new expected images"]
fn scalar9_dominates_ieee_for_named_empty_and_noop_targets_in_both_group_orders() {
    let ieee = genuine_ieee_update::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    });
    let mut contexts = 0;
    for case in ieee.into_iter().filter(|case| !case.subnormal_to_special) {
        for empty in [false, true] {
            for reverse in [false, true] {
                let mut requests: Vec<_> = case
                    .groups()
                    .into_iter()
                    .map(|group| (group.column, vec![(2, group.expected, group.replacement)]))
                    .collect();
                requests.push((
                    "ByteValue",
                    if empty {
                        vec![]
                    } else {
                        vec![(2, Value::Byte(255), Value::Byte(255))]
                    },
                ));
                if reverse {
                    requests.reverse();
                }
                assert_scalar_join(&case.before, &case.after, &requests);
                contexts += 1;
            }
        }
    }
    for case in all()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Endpoints)
    {
        for empty in [false, true] {
            for reverse in [false, true] {
                let mut requests: Vec<_> = case
                    .requests()
                    .into_iter()
                    .map(|(column, before, after)| (column, vec![(2, before, after)]))
                    .collect();
                let exact = Value::Single(Single::from_bits(0x8000_0000));
                requests.push((
                    "SingleValue",
                    if empty {
                        vec![]
                    } else {
                        vec![(2, exact.clone(), exact)]
                    },
                ));
                if reverse {
                    requests.reverse();
                }
                assert_scalar_join(&case.before, &case.after, &requests);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 16);
}

//! Indexed GUID requests retain the original journal and independent source images.
use super::{Storage, budget, model as journals, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, RecoverableNativeEditPlan, WriteBlocker, qualify_native_update_recovery,
    read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::{
        genuine_indexed_guid_update::{
            raw,
            requests::{self as source, Case},
        },
        original_fixed_wire::{self, Dialect, Group},
    },
    support::ace::resources::assert_public_boundary,
};

fn all() -> Vec<Case> {
    source::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn binding(before: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    digest.finalize().into()
}

fn group(
    updates: Vec<(usize, Value, Value)>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeFixedIntentGroup, NativeIntentError> {
    let updates = updates
        .into_iter()
        .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
        .collect();
    NativeFixedIntentGroup::new_ordinary_guid("Group", updates, work, cancelled)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false).unwrap()
}

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    let group = group(case.updates(), work, cancelled).map_err(RecoverableEditError::Intent)?;
    let image = NativeImage::capture(&mut Cursor::new(&case.before), scope(), work, cancelled)
        .map_err(|error| RecoverableEditError::Edit(EditError::Inspect(error)))?;
    image.plan_recoverable_ordinary_fixed_updates("Items", vec![group], work, cancelled)
}

fn commit(
    plan: RecoverableNativeEditPlan,
    before: &[u8],
    after: &[u8],
    count: usize,
    literal: &[u8],
) -> Vec<u8> {
    assert_eq!(plan.after_bytes(), after);
    assert_eq!(plan.changed_rows(), count);
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16OrdinaryGuidFixed
    );
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        literal
    );
    let decoded = NativeUpdateIntent::decode(literal, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal);

    let mut target = Storage::new(before.to_vec());
    let mut log = Storage::new(Vec::new());
    assert_eq!(
        plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap(),
        count
    );
    assert_eq!(target.bytes.get_ref(), after);
    assert_eq!(
        log.bytes.get_ref(),
        &journals::journal(before, after, journals::Marker::Committed)
    );
    target.bytes.into_inner()
}

#[test]
#[ignore = "four pinned originals, 28 ordered requests and 24 independent AFTERs"]
fn indexed_guid10_commits_original_ordered_intents_and_exact_images() {
    let mut count = 0;
    for case in all() {
        let literal = case.wire(binding(&case.before));
        let saved = commit(
            plan(&case, &mut budget(), &|| false).unwrap(),
            &case.before,
            &case.after,
            case.changed_rows(),
            &literal,
        );
        assert_eq!(
            read_table(
                &mut image(&saved).reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| { false }
            )
            .unwrap()
            .rows(),
            case.rows(true)
        );
        count += 1;
    }
    assert_eq!(count, 28);
}

#[test]
#[ignore = "original presence, indexed target policy and stale carriers are checked before publication"]
fn indexed_guid10_noop_empty_stale_and_primary_target_guards() {
    let mut counts = [0; 4];
    for case in all().into_iter().filter(|case| case.kind == 1) {
        for updates in [
            vec![(0, Value::Guid(raw::guid(0)), Value::Guid(raw::guid(0)))],
            vec![(157, Value::Null, Value::Null)],
            vec![],
        ] {
            let group = group(updates, &mut budget(), &|| false).unwrap();
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), &*case.before);
            assert_eq!(plan.changed_rows(), 0);
            counts[0] += 1;
        }
        for (row, expected, replacement) in [
            (0, Value::Guid(raw::guid(1)), Value::Guid(raw::guid(0))),
            (157, Value::Guid(raw::guid(156)), Value::Null),
        ] {
            let group =
                group(vec![(row, expected, replacement)], &mut budget(), &|| false).unwrap();
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
        for (expected, replacement) in [
            (Value::Int64(0), Value::Guid(raw::guid(0))),
            (Value::Guid(raw::guid(0)), Value::Int64(0)),
        ] {
            let group = group(vec![(0, expected, replacement)], &mut budget(), &|| false).unwrap();
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
        for empty in [false, true] {
            for reverse in [false, true] {
                let guid = group(case.updates(), &mut budget(), &|| false).unwrap();
                let updates = if empty {
                    vec![]
                } else {
                    vec![FixedScalarUpdate::new(
                        0,
                        Value::Int32(1),
                        Value::Int32(159),
                    )]
                };
                let primary =
                    NativeFixedIntentGroup::new("ID", updates, &mut budget(), &|| false).unwrap();
                let groups = if reverse {
                    vec![primary, guid]
                } else {
                    vec![guid, primary]
                };
                assert_eq!(
                    image(&case.before)
                        .plan_recoverable_ordinary_fixed_updates(
                            "Items",
                            groups,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(RecoverableEditError::Edit(EditError::Unsupported(
                        WriteBlocker::IndexedColumn
                    )))
                );
                counts[3] += 1;
            }
        }
    }
    assert_eq!(counts, [12, 8, 8, 16]);
}

#[test]
#[ignore = "old closed dialects may decode an empty group without granting GUID target authority"]
fn indexed_guid10_old_dialects_keep_named_empty_guid_closed() {
    let mut count = 0;
    for case in all().into_iter().filter(|case| case.kind == 1) {
        for dialect in [Dialect::Ordinary, Dialect::Ieee, Dialect::Scalar] {
            let wire = original_fixed_wire::original_wire(
                dialect,
                "Items",
                &[Group {
                    column: "Group",
                    updates: &[],
                }],
                case.before.len(),
                binding(&case.before),
            );
            let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);
            let mut log = Storage::new(journals::journal(
                &case.before,
                &case.before,
                journals::Marker::Committed,
            ));
            log.fail_sync = true;
            let saved = log.bytes.get_ref().clone();
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::Edit(
                    EditError::Unsupported(WriteBlocker::IndexedColumn)
                ))
            ));
            assert_eq!(log.bytes.get_ref(), &saved);
            assert_eq!(log.writes, 0);
            assert!(log.fail_sync);
            count += 1;
        }
    }
    assert_eq!(count, 12);
}

#[test]
#[ignore = "complete capture, request framing, stage and fresh proof share all measured controls"]
fn indexed_guid10_public_resources_and_cancellation() {
    let mut count = 0;
    let cases = all();
    for case in cases
        .iter()
        .filter(|case| [3, 4, 5, 6].contains(&case.kind) && !case.reverse)
    {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                plan(case, work, cancelled).map_err(|error| match error {
                    RecoverableEditError::Edit(EditError::Inspect(error))
                    | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
                    | RecoverableEditError::Intent(NativeIntentError::Request(
                        EditError::Inspect(error),
                    )) => error,
                    other => panic!("unexpected public plan: {other:?}"),
                })
            },
            |plan| {
                assert_eq!(plan.after_bytes(), &*case.after);
                assert_eq!(plan.changed_rows(), case.changed_rows());
            },
            |plan| {
                assert_eq!(plan.after_bytes(), &*case.after);
                assert_eq!(plan.changed_rows(), case.changed_rows());
            },
        );
        count += 1;
    }
    // One original suffices to measure the distinct two-group allocation graph;
    // all four originals retain independent composite commit/save coverage.
    let case = cases
        .iter()
        .find(|case| case.header == 5 && !case.descending && case.kind == 1)
        .unwrap();
    let expected = case.composite_after();
    for reverse in [false, true] {
        assert_public_boundary(
            budget,
            |work, cancelled| {
                let mut operation =
                    || -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
                        let guid = group(case.updates(), work, cancelled)?;
                        let score = NativeFixedIntentGroup::new(
                            "Score",
                            vec![FixedScalarUpdate::new(
                                0,
                                Value::Int32(-64),
                                Value::Int32(-63),
                            )],
                            work,
                            cancelled,
                        )?;
                        let groups = if reverse {
                            vec![score, guid]
                        } else {
                            vec![guid, score]
                        };
                        let image = NativeImage::capture(
                            &mut Cursor::new(&case.before),
                            scope(),
                            work,
                            cancelled,
                        )
                        .map_err(|error| RecoverableEditError::Edit(EditError::Inspect(error)))?;
                        image.plan_recoverable_ordinary_fixed_updates(
                            "Items", groups, work, cancelled,
                        )
                    };
                operation().map_err(|error| match error {
                    RecoverableEditError::Edit(EditError::Inspect(error))
                    | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
                    | RecoverableEditError::Intent(NativeIntentError::Request(
                        EditError::Inspect(error),
                    )) => error,
                    other => panic!("unexpected public mixed plan: {other:?}"),
                })
            },
            |plan| {
                assert_eq!(plan.after_bytes(), expected);
                assert_eq!(plan.changed_rows(), 1);
            },
            |plan| {
                assert_eq!(plan.after_bytes(), expected);
                assert_eq!(plan.changed_rows(), 1);
            },
        );
        count += 1;
    }
    assert_eq!(count, 18);
}

#[test]
#[ignore = "representative present, clear and prefix-changing cold recovery retain TCNI10"]
fn indexed_guid10_cold_recovery_replays_original_intent_and_refuses_mixed_commits() {
    let mut count = 0;
    for case in all()
        .into_iter()
        .filter(|case| [1, 4, 5].contains(&case.kind))
    {
        let (_, data, _) = raw::geometry(case.header);
        let selected = if case.kind == 5 { data[1] } else { data[0] };
        let span = selected * raw::PAGE..(selected + 1) * raw::PAGE;
        let mut mixed = case.before.to_vec();
        mixed[span.clone()].copy_from_slice(&case.after[span]);
        assert_ne!(mixed, &*case.before);
        assert_ne!(mixed, &*case.after);
        for (marker, original, mismatch) in [
            (journals::Marker::Pending, case.before.to_vec(), false),
            (journals::Marker::Partial, mixed.clone(), false),
            (journals::Marker::Committed, case.after.to_vec(), false),
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
                    if committed {
                        &*case.after
                    } else {
                        &*case.before
                    }
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
            count += 1;
        }
    }
    assert_eq!(count, 48);
}

#[test]
#[ignore = "indexed GUID and ordinary Score compose atomically in both written group orders"]
fn indexed_guid10_composes_score_without_discarding_other_group_fields() {
    let mut count = 0;
    for case in all().into_iter().filter(|case| case.kind == 1) {
        for reverse in [false, true] {
            let guid_updates = case.updates();
            let score_updates = [(0, Value::Int32(-64), Value::Int32(-63))];
            let guid = group(guid_updates.clone(), &mut budget(), &|| false).unwrap();
            let score = NativeFixedIntentGroup::new(
                "Score",
                vec![FixedScalarUpdate::new(
                    0,
                    Value::Int32(-64),
                    Value::Int32(-63),
                )],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let groups = if reverse {
                vec![score, guid]
            } else {
                vec![guid, score]
            };
            let guid_wire = Group {
                column: "Group",
                updates: &guid_updates,
            };
            let score_wire = Group {
                column: "Score",
                updates: &score_updates,
            };
            let wire_groups = if reverse {
                [score_wire, guid_wire]
            } else {
                [guid_wire, score_wire]
            };
            let literal = original_fixed_wire::original_wire(
                Dialect::Guid,
                "Items",
                &wire_groups,
                case.before.len(),
                binding(&case.before),
            );
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates("Items", groups, &mut budget(), &|| false)
                .unwrap();
            let _ = commit(plan, &case.before, &case.composite_after(), 1, &literal);
            count += 1;
        }
    }
    assert_eq!(count, 8);
}

#[test]
#[ignore = "malformed original key grammar, alias binding, owned map and linked root are never saved"]
fn indexed_guid10_original_preflight_refuses_malformed_grammar_and_ownership() {
    let mut contexts = 0;
    for case in all().into_iter().filter(|case| case.kind == 1) {
        let (definition, _, roots) = raw::geometry(case.header);
        let root = roots[1] * raw::PAGE;
        let present = root + 480 + if case.descending { 0 } else { 5 };
        assert_eq!(case.before[present + 9], 9);
        assert_eq!(
            case.before[present + 18],
            if case.descending { 0xf7 } else { 8 }
        );
        assert_eq!(
            &case.before[root + 4..root + 8],
            &(definition as u32).to_le_bytes()
        );
        assert_eq!(&case.before[root + 12..root + 20], &[0; 8]);

        // Literal original layout: column names end at192, two52-byte physical
        // descriptors precede four28-byte logical aliases. None is inferred
        // from a producer or from an AFTER image.
        let physical = definition * raw::PAGE + 192;
        let logical = physical + 104;
        assert_eq!(
            &case.before[physical + 52 + 38..physical + 52 + 42],
            &(roots[1] as u32).to_le_bytes()
        );
        assert_eq!(
            &case.before[logical + 28 + 8..logical + 28 + 12],
            &0_u32.to_le_bytes()
        );
        let map_reference = ((definition + 1) as u32) << 8 | 3;
        let map_page = (map_reference >> 8) as usize;
        let map_word = map_page * raw::PAGE + 14 + 6;
        let map_start = usize::from(u16::from_le_bytes(
            case.before[map_word..map_word + 2].try_into().unwrap(),
        ));
        let map_at = map_page * raw::PAGE + map_start;
        assert_eq!(before_map_base(&case.before, map_at), roots[1]);
        let map_bit = map_at + 5;
        assert_ne!(case.before[map_bit] & 1, 0);

        for variant in 0..7 {
            let mut before = case.before.to_vec();
            match variant {
                0 => before[present + 9] ^= 1,
                1 => before[present + 18] ^= 1,
                2 => before[root + 4..root + 8]
                    .copy_from_slice(&((definition + 1) as u32).to_le_bytes()),
                3 => before[root + 12..root + 16].copy_from_slice(&(roots[0] as u32).to_le_bytes()),
                4 => before[logical + 28 + 8..logical + 28 + 12]
                    .copy_from_slice(&2_u32.to_le_bytes()),
                5 => before[map_bit] &= !1,
                6 => before[root] = 5,
                _ => unreachable!(),
            }
            let empty = group(vec![], &mut budget(), &|| false).unwrap();
            assert!(
                image(&before)
                    .plan_recoverable_ordinary_fixed_updates(
                        "Items",
                        vec![empty],
                        &mut budget(),
                        &|| false
                    )
                    .is_err(),
                "h{} descending={} malformed={variant}",
                case.header,
                case.descending
            );
            contexts += 1;
        }
    }
    assert_eq!(contexts, 28);
}

fn before_map_base(bytes: &[u8], at: usize) -> usize {
    assert_eq!(bytes[at], 0);
    u32::from_le_bytes(bytes[at + 1..at + 5].try_into().unwrap()) as usize
}

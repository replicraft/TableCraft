//! Public TCNI4 planning/recovery retains independently authored counters/indexes.
use super::{Storage, budget, model as journals, planning_inspection, recovery_inspection, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, InspectError, NativeFixedIntentGroup, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::{self as sources, Case};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

struct Scenario {
    before: Vec<u8>,
    after: Vec<u8>,
    target: &'static str,
    expected: Value,
    replacement: Value,
    generated: bool,
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn scenario(header: u8, identity: Case) -> Scenario {
    let mut source = sources::source(identity);
    source.bytes[20] = header;
    assert!(source.ordinary);
    let expected = source.rows[0][source.target_position].clone();
    let mut after = source.bytes.clone();
    let base = 8 * 4096;
    let start = usize::from(u16::from_le_bytes(
        after[base + 14..base + 16].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    let replacement = match expected {
        Value::Int32(before) => {
            assert_eq!(source.target, "Score");
            assert_eq!(
                &after[base + start + 6..base + start + 10],
                &before.to_le_bytes()
            );
            after[base + start + 6..base + start + 10].copy_from_slice(&17_i32.to_le_bytes());
            Value::Int32(17)
        }
        Value::Boolean(true) => {
            assert_eq!(source.target, "Flag");
            assert_eq!(&after[base + start..base + start + 2], &[5, 0]);
            assert_eq!(after[9 * 4096 - 1] & 0x10, 0x10);
            after[9 * 4096 - 1] &= !0x10;
            Value::Boolean(false)
        }
        _ => panic!("independent original target must be Score or true Flag"),
    };
    Scenario {
        before: source.bytes,
        after,
        target: source.target,
        expected,
        replacement,
        generated: source.last_counter.is_some(),
    }
}

fn groups(case: &Scenario) -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new(
            case.target,
            vec![FixedScalarUpdate::new(
                0,
                case.expected.clone(),
                case.replacement.clone(),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn carrier(wire: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Int32(value) => {
            wire.push(2);
            wire.extend_from_slice(&u64::from(*value as u32).to_le_bytes());
        }
        Value::Boolean(value) => {
            wire.push(1);
            wire.extend_from_slice(&u64::from(*value).to_le_bytes());
        }
        _ => panic!("literal source uses no other intent carrier"),
    }
}

fn literal_wire(case: &Scenario) -> Vec<u8> {
    let length = 72 + 5 + 12 + case.target.len() + 26;
    let mut wire = Vec::from(*b"TCNI0004");
    wire.extend_from_slice(&(length as u32).to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    wire.extend_from_slice(&digest.finalize());
    for value in [5_u32, 1, 1, 0] {
        wire.extend_from_slice(&value.to_le_bytes());
    }
    wire.extend_from_slice(b"Items");
    for value in [case.target.len() as u32, 1, 0] {
        wire.extend_from_slice(&value.to_le_bytes());
    }
    wire.extend_from_slice(case.target.as_bytes());
    wire.extend_from_slice(&0_u64.to_le_bytes());
    carrier(&mut wire, &case.expected);
    carrier(&mut wire, &case.replacement);
    assert_eq!(wire.len(), length);
    wire
}

fn original(case: &Scenario) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&literal_wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
fn ordinary_retained_indexes_commit_eighteen_full_images_with_original_v4_intents() {
    let mut count = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            if !sources::source(identity).ordinary {
                continue;
            }
            let case = scenario(header, identity);
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.changed_rows(), 1);
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                literal_wire(&case)
            );
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            count += 1;
        }
    }
    assert_eq!(count, 18);
}

#[test]
fn retained_counter_index_recovery_recaptures_before_and_confirms_without_writes() {
    for header in [5, 6] {
        for identity in [
            Case::AliasHistory,
            Case::RetainedTree,
            Case::RelocatedPrimary,
            Case::InactiveIndexTail,
            Case::NullableOrdinary,
        ] {
            let case = scenario(header, identity);
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(case.after.clone());
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        original(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .recover(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        outcome
                    );
                    assert_eq!(
                        target.bytes.get_ref(),
                        if committed { &case.after } else { &case.before }
                    );
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    original(&case),
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
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

#[test]
fn empty_v4_manual_indexed_groups_commit_but_generated_id_still_refuses() {
    for header in [5, 6] {
        for identity in sources::CASES {
            if !sources::source(identity).ordinary {
                continue;
            }
            let case = scenario(header, identity);
            let group =
                NativeFixedIntentGroup::new(case.target, Vec::new(), &mut budget(), &|| false)
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
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert!(
                plan.intent().fixed().unwrap().groups()[0]
                    .updates()
                    .is_empty()
            );
            let id =
                NativeFixedIntentGroup::new("Id", Vec::new(), &mut budget(), &|| false).unwrap();
            let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                "Items",
                vec![id],
                &mut budget(),
                &|| false,
            );
            if case.generated {
                assert!(matches!(
                    result,
                    Err(RecoverableEditError::Edit(EditError::Unsupported(
                        WriteBlocker::Column
                    )))
                ));
            } else {
                let plan = result.unwrap();
                assert_eq!(plan.before_bytes(), case.before);
                assert_eq!(plan.after_bytes(), case.before);
                assert_eq!(plan.changed_rows(), 0);
                assert_eq!(plan.intent().fixed().unwrap().groups()[0].column(), "Id");
                assert!(
                    plan.intent().fixed().unwrap().groups()[0]
                        .updates()
                        .is_empty()
                );
                let mut target = Storage::new(case.before.clone());
                let mut journal = Storage::new(Vec::new());
                let prepared = plan
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 0);
                assert_eq!(target.bytes.get_ref(), &case.before);
                assert_eq!(
                    journal.bytes.get_ref(),
                    &journals::journal(&case.before, &case.before, Marker::Committed,)
                );
            }
        }
    }
}

#[test]
fn recovery_rejects_last_counter_tail_and_original_request_forgeries() {
    for header in [5, 6] {
        for variant in 0..3 {
            let mut case = scenario(header, Case::InactiveIndexTail);
            let mut intent = original(&case);
            match variant {
                0 => case.after[7 * 4096 + 20] ^= 1,
                1 => case.after[9 * 4096 + 498] ^= 1,
                2 => {
                    case.replacement = Value::Int32(18);
                    intent = original(&case);
                }
                _ => unreachable!(),
            }
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Pending,
            ));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

fn execute(
    case: &Scenario,
    recovery: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if !recovery {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", groups(case), work, cancelled)
            .map_err(planning_inspection)?;
        assert_eq!(plan.after_bytes(), case.after);
        return Ok(());
    }
    let mut journal = Storage::new(journals::journal(
        &case.before,
        &case.after,
        Marker::Pending,
    ));
    let saved = journal.bytes.get_ref().clone();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let result = qualify_native_update_recovery(checked, original(case), scope(), work, cancelled)
        .map(drop)
        .map_err(recovery_inspection);
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &saved);
    result
}

#[test]
fn retained_counter_index_planning_and_recovery_obey_exact_pools_and_cancel() {
    let case = scenario(6, Case::AliasHistory);
    for recovery in [false, true] {
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0);
        execute(&case, recovery, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let costs = std::array::from_fn::<_, 3, _>(|index| {
            initial[index] - measured.remaining(POOLS[index])
        });
        assert!(costs.iter().all(|cost| *cost > 0));
        let terminal = polls.get();
        for variant in 0..6 {
            let mut limits = costs;
            if variant < 3 {
                limits[variant] -= 1;
            }
            let mut work = ResourceBudget::new(limits[0], limits[1], limits[2]);
            polls.set(0);
            let result = execute(&case, recovery, &mut work, &|| {
                polls.set(polls.get() + 1);
                variant == 4 || (variant == 5 && polls.get() == terminal)
            });
            if variant == 3 {
                result.unwrap();
                for resource in POOLS {
                    assert_eq!(work.remaining(resource), 0);
                }
            } else if variant < 3 {
                assert!(
                    matches!(result, Err(InspectError::Limit(error)) if error.resource == POOLS[variant])
                );
            } else {
                assert_eq!(result, Err(InspectError::Cancelled));
            }
        }
    }
}

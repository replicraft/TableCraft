//! Public normal/journal consumers use independently authored high-window inputs.
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
use tablecraft_testkit::access::canonical_ordinary_inline::{self as sources, Case};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

struct Scenario {
    before: Vec<u8>,
    after: Vec<u8>,
    expected: i32,
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn scenario(header: u8, case: Case) -> Scenario {
    let mut source = sources::source(case);
    source.bytes[20] = header;
    assert_eq!(source.table, "Items");
    assert_eq!(source.columns, ["Id", "Score"]);
    assert_eq!(source.data, 95);
    assert_eq!(source.bytes.len(), 96 * 4096);
    let base = 95 * 4096;
    let start = usize::from(u16::from_le_bytes(
        source.bytes[base + 14..base + 16].try_into().unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    let expected = source.rows[0][1];
    let payload = base + start + 6;
    assert_eq!(&source.bytes[payload..payload + 4], &expected.to_le_bytes());
    let mut after = source.bytes.clone();
    after[payload..payload + 4].copy_from_slice(&17_i32.to_le_bytes());
    Scenario {
        before: source.bytes,
        after,
        expected,
    }
}

fn groups(case: &Scenario) -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new(
            "Score",
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(case.expected),
                Value::Int32(17),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn literal_wire(case: &Scenario) -> Vec<u8> {
    // Existing V4 header/body framing, independent of production codec output.
    let mut wire = Vec::from(*b"TCNI0004");
    wire.extend_from_slice(&120_u32.to_le_bytes());
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
    for value in [5_u32, 1, 0] {
        wire.extend_from_slice(&value.to_le_bytes());
    }
    wire.extend_from_slice(b"Score");
    wire.extend_from_slice(&0_u64.to_le_bytes());
    wire.push(2);
    wire.extend_from_slice(&u64::from(case.expected as u32).to_le_bytes());
    wire.push(2);
    wire.extend_from_slice(&17_u64.to_le_bytes());
    assert_eq!(wire.len(), 120);
    wire
}

fn original(case: &Scenario) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&literal_wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
fn dynamic_inline_plans_commit_ten_complete_images_with_unchanged_v4_framing() {
    let mut count = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            let source = sources::source(identity);
            if !source.ordinary {
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
    assert_eq!(count, 10);
}

#[test]
fn dynamic_inline_recovery_rederives_original_windows_and_confirms_without_writes() {
    for header in [5, 6] {
        for identity in [Case::DifferentWindows, Case::FutureFree, Case::WideExtrema] {
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
fn dynamic_inline_invalid_roles_refuse_planning_and_recovery_before_effects() {
    let mut count = 0;
    for header in [5, 6] {
        for identity in sources::CASES {
            let source = sources::source(identity);
            if source.ordinary {
                continue;
            }
            let case = scenario(header, identity);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
            ));
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Pending,
            ));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    original(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::Edit(
                    EditError::Unsupported(WriteBlocker::ActiveContent)
                ))
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            count += 1;
        }
    }
    assert_eq!(count, 14);
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
fn dynamic_inline_complete_work_has_exact_pools_and_terminal_cancellation() {
    let case = scenario(6, Case::DifferentWindows);
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
        assert!(terminal > 1);
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
                assert!(matches!(
                    result,
                    Err(InspectError::Limit(error)) if error.resource == POOLS[variant]
                ));
            } else {
                assert_eq!(result, Err(InspectError::Cancelled));
            }
        }
    }
}

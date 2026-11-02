//! Public ordinary-content planning and recovery over independent SOURCE bytes.
use super::{Storage, budget, model as journals, planning_inspection, recovery_inspection, scope};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, InspectError, NativeFixedEditIntent, NativeFixedIntentGroup,
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent,
    NativeUpdateProfile, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, LegacyDateTime, Value},
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{self as sources, Case};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

struct Scenario {
    before: Vec<u8>,
    after: Vec<u8>,
    table: String,
    requests: Vec<(String, Value, Value)>,
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn first_record(bytes: &[u8], page: u32) -> usize {
    let base = page as usize * 4096;
    let word = u16::from_le_bytes(bytes[base + 14..base + 16].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    base + usize::from(word)
}

fn scenario(header: u8, case: Case) -> Scenario {
    let mut source = sources::source(case);
    assert!(source.ordinary);
    source.bytes[20] = header;
    let target = source
        .tables
        .iter()
        .find(|table| !table.rows.is_empty())
        .unwrap();
    let Value::Int32(before) = target.rows[0][0] else {
        panic!("the selected independent SOURCE column must be Int32");
    };
    let mut after = source.bytes.clone();
    let start = first_record(&source.bytes, target.data[0]);
    assert_eq!(&after[start + 2..start + 6], &before.to_le_bytes());
    after[start + 2..start + 6].copy_from_slice(&73_i32.to_le_bytes());
    Scenario {
        before: source.bytes,
        after,
        table: target.name.clone(),
        requests: vec![(
            target.columns[0].clone(),
            Value::Int32(before),
            Value::Int32(73),
        )],
    }
}

fn wide(header: u8) -> Scenario {
    let mut source = sources::source(Case::InactiveXml);
    source.bytes[20] = header;
    let mut after = source.bytes.clone();
    let start = first_record(&source.bytes, 8);
    assert_eq!(&after[start + 9..start + 17], b"<Event/>");
    assert_eq!(&after[start + 17..start + 25], &1_i64.to_le_bytes());
    assert_eq!(&after[start + 37..start + 45], &0_u64.to_le_bytes());
    assert_eq!(after[start + 61] & 0x11, 0);
    after[start + 9..start + 17].copy_from_slice(&7_i64.to_le_bytes());
    after[start + 17..start + 25].copy_from_slice(&(-10001_i64).to_le_bytes());
    after[start + 37..start + 45].copy_from_slice(&0x8000_0000_0000_0000_u64.to_le_bytes());
    after[start + 61] |= 0x11;
    Scenario {
        before: source.bytes,
        after,
        table: "Items".into(),
        requests: vec![
            ("Flag".into(), Value::Boolean(false), Value::Boolean(true)),
            ("Int64Value".into(), Value::Null, Value::Int64(7)),
            (
                "Money".into(),
                Value::Currency(Currency::from_scaled(1)),
                Value::Currency(Currency::from_scaled(-10001)),
            ),
            (
                "DateValue".into(),
                Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                Value::DateTime(LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap()),
            ),
        ],
    }
}

fn groups(case: &Scenario) -> Vec<NativeFixedIntentGroup> {
    case.requests
        .iter()
        .map(|(column, before, after)| {
            NativeFixedIntentGroup::new(
                column,
                vec![FixedScalarUpdate::new(0, before.clone(), after.clone())],
                &mut budget(),
                &|| false,
            )
            .unwrap()
        })
        .collect()
}

fn original(case: &Scenario) -> NativeUpdateIntent {
    // A decoded TCNI0004 record is DATA, never an absence certificate. Its
    // complete BEFORE and original groups must qualify again during recovery.
    let fixed = NativeFixedEditIntent::new(
        &case.table,
        groups(case),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut wire = fixed.encode(&mut budget(), &|| false).unwrap();
    wire[..8].copy_from_slice(b"TCNI0004");
    wire[12..14].copy_from_slice(&4_u16.to_le_bytes());
    NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap()
}

#[test]
fn ordinary_public_plans_commit_literal_images_and_record_exact_original_groups() {
    let mut count = 0;
    for header in [5, 6] {
        let mut cases = [
            Case::Extrema,
            Case::Named,
            Case::ProgramLikeName,
            Case::XmlData,
            Case::Multiple,
            Case::ReorderedCatalog,
            Case::RelocatedUser,
            Case::RelocatedMaps,
            Case::SplitData,
            Case::TrailingFreePage,
        ]
        .into_iter()
        .map(|case| scenario(header, case))
        .collect::<Vec<_>>();
        cases.push(wide(header));
        for case in cases {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    &case.table,
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
            assert!(plan.intent().int32().is_none());
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0004");
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            let recorded = decoded.fixed().unwrap().groups();
            assert_eq!(recorded.len(), case.requests.len());
            for (group, (name, before, after)) in recorded.iter().zip(&case.requests) {
                assert_eq!(group.column(), name);
                assert_eq!(group.updates().len(), 1);
                assert_eq!(group.updates()[0].row(), 0);
                assert_eq!(group.updates()[0].expected(), before);
                assert_eq!(group.updates()[0].value(), after);
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed,)
            );
            count += 1;
        }
    }
    assert_eq!(count, 22);
}

#[test]
fn ordinary_recovery_reconstructs_pending_partial_and_committed_images_repeatedly() {
    for header in [5, 6] {
        for case in [scenario(header, Case::RelocatedMaps), wide(header)] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                let mut target = Storage::new(case.after.clone());
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
                        if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        }
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
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

#[test]
fn ordinary_recovery_refuses_altered_after_wrong_request_source_and_positive_program() {
    for header in [5, 6] {
        for variant in 0..4 {
            let mut case = scenario(header, Case::Extrema);
            let mut intent = original(&case);
            match variant {
                0 => case.after[200] ^= 1,
                1 => {
                    case.requests[0].2 = Value::Int32(74);
                    intent = original(&case);
                }
                2 => {
                    // Intent remains bound to the original header5/6 image.
                    case.before[20] = if header == 5 { 6 } else { 5 };
                    case.after[20] = case.before[20];
                }
                3 => {
                    let mut source = sources::source(Case::PositiveProgram);
                    source.bytes[20] = header;
                    case.before = source.bytes;
                    case.after = case.before.clone();
                    case.requests[0].1 = Value::Int32(1);
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
            let error =
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .err()
                    .expect("unqualified original request/image pair");
            match variant {
                0 | 1 => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::ImageMismatch
                )),
                2 => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::Intent(NativeIntentError::BeforeMismatch)
                )),
                3 => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::Edit(EditError::Unsupported(
                        WriteBlocker::ActiveContent
                    ))
                )),
                _ => unreachable!(),
            }
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
fn ordinary_planning_and_recovery_exact_pools_and_terminal_cancellation_precede_effects() {
    let case = wide(6);
    for recovery in [false, true] {
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0);
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let saved = journal.bytes.get_ref().clone();
        if recovery {
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            drop(
                qualify_native_update_recovery(
                    checked,
                    original(&case),
                    scope(),
                    &mut measured,
                    &|| {
                        polls.set(polls.get() + 1);
                        false
                    },
                )
                .unwrap(),
            );
        } else {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    &case.table,
                    groups(&case),
                    &mut measured,
                    &|| {
                        polls.set(polls.get() + 1);
                        false
                    },
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.after);
        }
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
            let cancelled = || {
                polls.set(polls.get() + 1);
                variant == 4 || (variant == 5 && polls.get() == terminal)
            };
            let error = if recovery {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let result = qualify_native_update_recovery(
                    checked,
                    original(&case),
                    scope(),
                    &mut work,
                    &cancelled,
                );
                if variant == 3 {
                    drop(result.unwrap());
                    continue;
                }
                recovery_inspection(result.err().unwrap())
            } else {
                let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
                    &case.table,
                    groups(&case),
                    &mut work,
                    &cancelled,
                );
                if variant == 3 {
                    assert_eq!(result.unwrap().after_bytes(), case.after);
                    continue;
                }
                planning_inspection(result.err().unwrap())
            };
            if variant < 3 {
                assert!(
                    matches!(error, InspectError::Limit(error) if error.resource == POOLS[variant])
                );
            } else {
                assert_eq!(error, InspectError::Cancelled);
            }
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
fn ordinary_prepare_and_recovery_refuse_third_images_before_target_writes() {
    let case = wide(6);
    let plan = image(&case.before)
        .plan_recoverable_ordinary_fixed_updates(&case.table, groups(&case), &mut budget(), &|| {
            false
        })
        .unwrap();
    let mut target = Storage::new(case.before.clone());
    target.bytes.get_mut()[200] ^= 1;
    let retained = target.bytes.get_ref().clone();
    let mut journal = Storage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &retained);

    let mut journal = Storage::new(journals::journal(
        &case.before,
        &case.after,
        Marker::Pending,
    ));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified =
        qualify_native_update_recovery(checked, original(&case), scope(), &mut budget(), &|| false)
            .unwrap();
    assert_eq!(
        qualified.recover(&mut target, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &retained);
}

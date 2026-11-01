//! Source-qualified literal fitting images; no planner output supplies expectations.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
    qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::canonical_ordinary_fitted::{self as sources, Case, Recipe};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFittedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let updates = if matches!(group.number, 1 | 2) {
                NativeFittedUpdates::Variable(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            SlotScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            } else {
                NativeFittedUpdates::Fixed(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            FixedScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            };
            NativeFittedIntentGroup::new(group.column, updates, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

// Independent public wire grammar; no production format constants or codec.
fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, payload) = match value {
        Value::Null => (0, vec![]),
        Value::Boolean(value) => (1, vec![u8::from(*value)]),
        Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
        Value::Int64(value) => (3, value.to_le_bytes().to_vec()),
        Value::Currency(value) => (4, value.scaled().to_le_bytes().to_vec()),
        Value::DateTime(value) => (5, value.to_bits().to_le_bytes().to_vec()),
        Value::Text(value) => (6, value.as_bytes().to_vec()),
        Value::Binary(value) => (7, value.clone()),
        _ => panic!("only original supported source carriers enter profile5"),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(&payload);
}

fn wire(case: &Case) -> Vec<u8> {
    let mut body = Vec::from(*b"Items");
    let mut assignments = 0;
    for group in &case.groups {
        let mut cells = Vec::new();
        for update in &group.updates {
            let mut cell = Vec::new();
            cell.extend_from_slice(&(update.row as u64).to_le_bytes());
            carrier(&mut cell, &update.expected);
            carrier(&mut cell, &update.replacement);
            cells.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
            cells.extend_from_slice(&cell);
            assignments += 1;
        }
        let total = 20 + group.column.len() + cells.len();
        body.extend_from_slice(&(total as u32).to_le_bytes());
        body.extend_from_slice(&(group.column.len() as u32).to_le_bytes());
        body.extend_from_slice(&(group.updates.len() as u32).to_le_bytes());
        body.extend_from_slice(&[u8::from(matches!(group.number, 1 | 2)), 0, 0, 0]);
        body.extend_from_slice(&0_u32.to_le_bytes());
        body.extend_from_slice(group.column.as_bytes());
        body.extend_from_slice(&cells);
    }
    let mut wire = Vec::from(*b"TCNI0005");
    wire.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    wire.extend_from_slice(&[5, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    wire.extend_from_slice(&digest.finalize());
    for count in [5_u32, case.groups.len() as u32, assignments, 0] {
        wire.extend_from_slice(&count.to_le_bytes());
    }
    wire.extend_from_slice(&body);
    wire
}

fn original(intent: &NativeUpdateIntent, case: &Case) {
    assert_eq!(intent.profile(), NativeUpdateProfile::Ace16OrdinaryFitted);
    assert!(intent.int32().is_none());
    assert!(intent.fixed().is_none());
    let fitted = intent.fitted().unwrap();
    assert_eq!(fitted.table(), "Items");
    assert_eq!(fitted.before_length(), case.before.len() as u64);
    assert_eq!(fitted.groups().len(), case.groups.len());
    for (group, expected) in fitted.groups().iter().zip(&case.groups) {
        assert_eq!(group.column(), expected.column);
        match group.updates() {
            NativeFittedUpdates::Variable(updates) => {
                assert!(matches!(expected.number, 1 | 2));
                assert_eq!(updates.len(), expected.updates.len());
                for (actual, expected) in updates.iter().zip(&expected.updates) {
                    assert_eq!(actual.row(), expected.row);
                    assert_eq!(actual.expected(), &expected.expected);
                    assert_eq!(actual.value(), &expected.replacement);
                }
            }
            NativeFittedUpdates::Fixed(updates) => {
                assert!(!matches!(expected.number, 1 | 2));
                assert_eq!(updates.len(), expected.updates.len());
                for (actual, expected) in updates.iter().zip(&expected.updates) {
                    assert_eq!(actual.row(), expected.row);
                    assert_eq!(actual.expected(), &expected.expected);
                    assert_eq!(actual.value(), &expected.replacement);
                }
            }
        }
    }
}

#[test]
fn ordinary_fitted_commits_thirty_literal_images_with_exact_profile_five_originals() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in sources::RECIPES {
            let case = sources::source(header, recipe);
            let Some(after) = &case.expected else {
                continue;
            };
            sources::assert_complement(&case);
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), after);
            assert_eq!(plan.changed_rows(), case.changed);
            original(plan.intent(), &case);
            let expected_wire = wire(&case);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected_wire
            );
            let decoded =
                NativeUpdateIntent::decode(&expected_wire, &mut budget(), &|| false).unwrap();
            original(&decoded, &case);
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                prepared.commit(&mut budget(), &|| false).unwrap(),
                case.changed
            );
            assert_eq!(target.bytes.get_ref(), after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, after, Marker::Committed)
            );
            count += 1;
        }
    }
    assert_eq!(count, 30);
}

fn recover(case: &Case, marker: Marker, initial: Vec<u8>) {
    let after = case.expected.as_ref().unwrap();
    let committed = matches!(marker, Marker::Committed);
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, after, marker));
    let journal_before = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let intent = NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            outcome
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed { after } else { &case.before }
        );
    }
    if committed {
        assert_eq!(target.writes, 0);
    }
    let writes = target.writes;
    target.fail_prefix = Some(0);
    let intent = NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified =
        qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false).unwrap();
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        outcome
    );
    assert_eq!(target.writes, writes);
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &journal_before);
}

#[test]
fn fitted_recovery_reconstructs_null_shared_fixed_noop_and_empty_requests() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::NullTextToEmpty,
            Recipe::SharedExactFit,
            Recipe::JointFiveFixed,
            Recipe::DisjointRows,
            Recipe::ExactNoops,
            Recipe::EmptyGroups,
        ] {
            let case = sources::source(header, recipe);
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                recover(&case, marker, case.expected.as_ref().unwrap().clone());
                count += 1;
            }
        }
    }
    assert_eq!(count, 36);
}

#[test]
fn fitted_pending_recovery_rolls_back_truly_mixed_changed_record_bytes() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::SharedExactFit,
            Recipe::JointFiveFixed,
            Recipe::DisjointRows,
        ] {
            let case = sources::source(header, recipe);
            let after = case.expected.as_ref().unwrap();
            let differences = case
                .before
                .iter()
                .zip(after)
                .enumerate()
                .filter_map(|(at, (before, after))| (before != after).then_some(at))
                .collect::<Vec<_>>();
            assert!(differences.len() > 1);
            let mut partial = case.before.clone();
            for at in differences.iter().step_by(2) {
                partial[*at] = after[*at];
            }
            assert_ne!(partial, case.before);
            assert_ne!(&partial, after);
            for marker in [Marker::Pending, Marker::Partial] {
                recover(&case, marker, partial.clone());
                count += 1;
            }
        }
    }
    assert_eq!(count, 12);
}

#[test]
fn fitted_recovery_refuses_self_consistent_after_and_original_request_forgeries() {
    let mut count = 0;
    for header in [5, 6] {
        for variant in 0..6 {
            let mut case = sources::source(header, Recipe::CompressedText);
            let frame = &case.frames[0];
            let mut after = case.expected.as_ref().unwrap().clone();
            match variant {
                0 => after[frame.absolute + 6] ^= 1,  // unrelated Large sibling
                1 => after[frame.absolute + 60] ^= 1, // inactive slack after final EOD57
                2 => after[7 * 4096 + 16] ^= 1,       // TD row count
                3 => after[frame.absolute + frame.length - 1] ^= 0x20, // unrelated presence bit
                4 => after[frame.absolute + frame.length - 3] ^= 1, // variable count
                5 => case.groups[0].updates[0].replacement = Value::Text("zeta".into()),
                _ => unreachable!(),
            }
            let expected_wire = wire(&case);
            let intent =
                NativeUpdateIntent::decode(&expected_wire, &mut budget(), &|| false).unwrap();
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false,),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            count += 1;
        }
    }
    assert_eq!(count, 12);
}

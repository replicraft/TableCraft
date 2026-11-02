//! Retained original files plus frozen source-derived AFTERs, never writer goldens.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, SlotScalarUpdate, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_property_lval::{self as sources, Case, Recipe};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFittedIntentGroup> {
    let updates = if case.empty {
        Vec::new()
    } else {
        vec![SlotScalarUpdate::new(
            0,
            case.expected.clone(),
            case.replacement.clone(),
        )]
    };
    vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, payload) = match value {
        Value::Null => (0, &[][..]),
        Value::Text(value) => (6, value.as_bytes()),
        _ => panic!("only literal original Marker carriers"),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(payload);
}

// Independent profile5 framing, including a real retained empty Variable group.
fn wire(case: &Case) -> Vec<u8> {
    let mut updates = Vec::new();
    if !case.empty {
        let mut cell = Vec::new();
        cell.extend_from_slice(&0_u64.to_le_bytes());
        carrier(&mut cell, &case.expected);
        carrier(&mut cell, &case.replacement);
        updates.extend_from_slice(&((4 + cell.len()) as u32).to_le_bytes());
        updates.extend_from_slice(&cell);
    }
    let count = u32::from(!case.empty);
    let mut body = Vec::from(*b"Sentinel");
    for value in [(20 + 6 + updates.len()) as u32, 6, count, 1, 0] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(b"Marker");
    body.extend_from_slice(&updates);
    let mut encoded = Vec::from(*b"TCNI0005");
    encoded.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    encoded.extend_from_slice(&[5, 0, 0, 0]);
    encoded.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    encoded.extend_from_slice(&digest.finalize());
    for value in [8_u32, 1, count, 0] {
        encoded.extend_from_slice(&value.to_le_bytes());
    }
    encoded.extend_from_slice(&body);
    encoded
}

fn intent(case: &Case) -> NativeUpdateIntent {
    let decoded = NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.profile(), NativeUpdateProfile::Ace16OrdinaryFitted);
    assert_eq!(decoded.fitted().unwrap().table(), "Sentinel");
    assert_eq!(decoded.fitted().unwrap().groups().len(), 1);
    decoded
}

#[test]
#[ignore = "requires both explicit property/LVAL original and expected directories"]
fn genuine_marker_twelve_literal_commits_keep_property_ole_and_complete_complement() {
    let mut committed = 0;
    for case in cases() {
        let Some(after) = &case.after else {
            continue;
        };
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "Sentinel",
                groups(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.changed_rows(), case.changed_rows);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        intent(&case);
        let start = case.data as usize * 4096 + 4030;
        assert_eq!(&after[..start], &case.before[..start]);
        assert_eq!(&after[start + 66..], &case.before[start + 66..]);
        assert_eq!(
            &after[case.property_offset..case.property_offset + 57],
            &case.before[case.property_offset..case.property_offset + 57]
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            prepared.commit(&mut budget(), &|| false).unwrap(),
            case.changed_rows
        );
        assert_eq!(target.bytes.get_ref(), after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, after, Marker::Committed)
        );
        committed += 1;
    }
    assert_eq!(committed, 12);
}

fn recover(case: &Case, marker: Marker, initial: Vec<u8>) {
    let after = case.after.as_ref().unwrap();
    let committed = matches!(marker, Marker::Committed);
    let expected = if committed { after } else { &case.before };
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, after, marker));
    let original_journal = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent(case), scope(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            outcome
        );
        assert_eq!(target.bytes.get_ref(), expected);
    }
    if committed {
        assert_eq!(target.writes, 0);
    }
    let writes = target.writes;
    target.fail_prefix = Some(0);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified =
        qualify_native_update_recovery(checked, intent(case), scope(), &mut budget(), &|| false)
            .unwrap();
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        outcome
    );
    assert_eq!(target.writes, writes);
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &original_journal);
}

#[test]
#[ignore = "requires both explicit property/LVAL original and expected directories"]
fn genuine_marker_recovery_rederives_original_intent_and_confirms_without_writes() {
    let mut states = 0;
    let mut mixed = 0;
    for case in cases() {
        let Some(after) = &case.after else {
            continue;
        };
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            recover(&case, marker, after.clone());
            states += 1;
        }
        if !matches!(case.recipe, Recipe::Same | Recipe::Shorter | Recipe::Clear) {
            continue;
        }
        let differences = case
            .before
            .iter()
            .zip(after)
            .enumerate()
            .filter_map(|(at, (old, new))| (old != new).then_some(at))
            .collect::<Vec<_>>();
        assert!(differences.len() > 1);
        let start = case.data as usize * 4096 + 4030;
        assert!(
            differences
                .iter()
                .all(|at| (start..start + 66).contains(at))
        );
        let mut interrupted = case.before.clone();
        for at in differences.iter().step_by(2) {
            interrupted[*at] = after[*at];
        }
        assert_ne!(interrupted, case.before);
        assert_ne!(&interrupted, after);
        for marker in [Marker::Pending, Marker::Partial] {
            recover(&case, marker, interrupted.clone());
            mixed += 1;
        }
    }
    assert_eq!(states, 36);
    assert_eq!(mixed, 12);
}

#[test]
#[ignore = "requires both explicit property/LVAL original and expected directories"]
fn genuine_marker_refuses_capacity_and_stale_expectations_without_preparation() {
    let mut refused = 0;
    for case in cases().into_iter().filter(|case| case.after.is_none()) {
        let result = image(&case.before).plan_recoverable_ordinary_fitted_updates(
            "Sentinel",
            groups(&case),
            &mut budget(),
            &|| false,
        );
        let expected = if case.recipe == Recipe::Overflow {
            EditError::Unsupported(WriteBlocker::RowLayout)
        } else {
            assert_eq!(case.recipe, Recipe::Stale);
            EditError::Conflict
        };
        assert!(matches!(result, Err(RecoverableEditError::Edit(error)) if error == expected));
        refused += 1;
    }
    assert_eq!(refused, 4);
}

#[test]
#[ignore = "requires both explicit property/LVAL original and expected directories"]
fn genuine_marker_recovery_rejects_self_consistent_ole_property_and_system_forgeries() {
    let mut refused = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Shorter)
    {
        for variant in 0..6 {
            let mut after = case.after.as_ref().unwrap().clone();
            let row = case.data as usize * 4096 + 4030;
            let ole = usize::from(u16::from_le_bytes(
                after[row + 59..row + 61].try_into().unwrap(),
            ));
            let changed = match variant {
                0 => row + ole + 12,            // readonly inline OLE value
                1 => case.property_offset + 56, // actual PublishToWeb value
                2 => 2 * 4096 + 16,             // system catalog definition
                3 => row + 2,                   // unrelated manual ID
                4 => row + 56,                  // stationary inactive original gap
                5 => row + 65,                  // unrelated presence bit
                _ => unreachable!(),
            };
            after[changed] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 12);
}

//! Public profile6 first growth and profile5 same-length forwarded continuation.
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
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::genuine_alternate_owned::{self as sources, Case};

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

fn groups(old: &str, new: &str) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new(
            "Marker",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                0,
                Value::Text(old.into()),
                Value::Text(new.into()),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn carrier(wire: &mut Vec<u8>, value: &str) {
    wire.extend_from_slice(&[6, 0, 0, 0]);
    wire.extend_from_slice(&(value.len() as u32).to_le_bytes());
    wire.extend_from_slice(value.as_bytes());
}

// The original request is independently framed, never copied from a plan intent.
fn wire(before: &[u8], old: &str, new: &str, profile: u8) -> Vec<u8> {
    let mut cell = Vec::new();
    cell.extend_from_slice(&0_u64.to_le_bytes());
    carrier(&mut cell, old);
    carrier(&mut cell, new);
    let mut body = Vec::from(*b"Sentinel");
    for number in [(20 + 6 + 4 + cell.len()) as u32, 6, 1, 1, 0] {
        body.extend_from_slice(&number.to_le_bytes());
    }
    body.extend_from_slice(b"Marker");
    body.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
    body.extend_from_slice(&cell);
    let mut output = Vec::from(if profile == 6 {
        *b"TCNI0006"
    } else {
        *b"TCNI0005"
    });
    output.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    output.extend_from_slice(&[profile, 0, 0, 0]);
    output.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    output.extend_from_slice(&digest.finalize());
    for number in [8_u32, 1, 1, 0] {
        output.extend_from_slice(&number.to_le_bytes());
    }
    output.extend_from_slice(&body);
    output
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(
        &wire(&case.before, sources::OLD, sources::GROWN, 6),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
#[ignore = "requires all pinned alternate-owned SOURCE, growth and second-edit images"]
fn genuine_alternate_owned_commits_exact_profile_six_then_fits_forwarded_profile_five() {
    for case in cases() {
        let fitting = image(&case.before).plan_recoverable_ordinary_fitted_updates(
            "Sentinel",
            groups(sources::OLD, sources::GROWN),
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            fitting,
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::RowLayout
            )))
        ));
        let automatic = image(&case.before)
            .plan_recoverable_ordinary_variable_updates(
                "Sentinel",
                groups(sources::OLD, sources::GROWN),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(automatic.after_bytes(), case.after);
        assert_eq!(
            automatic.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
        );
        assert_eq!(
            automatic.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before, sources::OLD, sources::GROWN, 6)
        );
        let plan = image(&case.before)
            .plan_recoverable_ordinary_owned_growth_updates(
                "Sentinel",
                groups(sources::OLD, sources::GROWN),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        let changed_pages = case
            .before
            .as_chunks::<4096>()
            .0
            .iter()
            .zip(case.after.as_chunks::<4096>().0.iter())
            .enumerate()
            .filter_map(|(page, (before, after))| (before != after).then_some(page as u32))
            .collect::<Vec<_>>();
        assert_eq!(changed_pages, [case.root, case.target]);
        assert_eq!(plan.before_bytes().len(), plan.after_bytes().len());
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before, sources::OLD, sources::GROWN, 6)
        );
        assert!(plan.intent().owned_growth().is_some());
        assert!(plan.intent().fitted().is_none());
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        let automatic = image(target.bytes.get_ref())
            .plan_recoverable_ordinary_variable_updates(
                "Sentinel",
                groups(sources::GROWN, sources::SECOND),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(automatic.after_bytes(), case.second);
        assert_eq!(
            automatic.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFitted
        );
        assert_eq!(
            automatic.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.after, sources::GROWN, sources::SECOND, 5)
        );
        let next = image(target.bytes.get_ref())
            .plan_recoverable_ordinary_fitted_updates(
                "Sentinel",
                groups(sources::GROWN, sources::SECOND),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(next.before_bytes(), case.after);
        assert_eq!(next.after_bytes(), case.second);
        assert_eq!(
            next.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFitted
        );
        assert_eq!(
            next.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.after, sources::GROWN, sources::SECOND, 5)
        );
        let mut second_journal = Storage::new(Vec::new());
        assert_eq!(
            next.prepare(&mut target, &mut second_journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.second);
    }
}

fn recover(case: &Case, marker: Marker, initial: Vec<u8>) {
    let committed = matches!(marker, Marker::Committed);
    let expected = if committed { &case.after } else { &case.before };
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
    let saved = journal.bytes.get_ref().clone();
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
    assert_eq!(journal.bytes.get_ref(), &saved);
}

#[test]
#[ignore = "requires all pinned alternate-owned SOURCE, growth and second-edit images"]
fn genuine_alternate_owned_recovery_handles_causal_partial_page_and_settled_images() {
    for case in cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            recover(&case, marker, case.after.clone());
        }
        // Each mixed image is a causal interrupted two-page commit: exactly
        // one complete changed DATA page reached storage and the other did not.
        for written in [case.root, case.target] {
            let base = written as usize * 4096;
            let mut mixed = case.before.clone();
            mixed[base..base + 4096].copy_from_slice(&case.after[base..base + 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            for marker in [Marker::Pending, Marker::Partial] {
                recover(&case, marker, mixed.clone());
            }
            // A committed marker cannot authorize redo over a partial target.
            // It must preserve both resources and report TargetMismatch.
            let mut target = Storage::new(mixed.clone());
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Committed,
            ));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(matches!(
                    qualified.recover(&mut target, &mut budget(), &|| false),
                    Err(JournalError::TargetMismatch)
                ));
                assert_eq!(target.bytes.get_ref(), &mixed);
            }
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualified.confirm_completed(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            ));
            assert_eq!(target.writes, 0);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
#[ignore = "requires all pinned alternate-owned SOURCE, growth and second-edit images"]
fn genuine_alternate_owned_replay_refuses_pointer_ole_property_and_free_extent_forgery() {
    for case in cases() {
        let base = case.root as usize * 4096;
        let target = case.target as usize * 4096;
        for at in [
            base + 4030,
            target + 4072 + 12,
            case.property_offset + 56,
            base + 1000,
            base + 16,
            target + 14,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

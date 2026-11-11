//! Empty native actions still reconstruct complete source and protocol authority.
use super::{budget, groups, literal};
use crate::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, WriteBlocker, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{field_changes, fields, joined_update},
    support::io::journal::MemoryStorage,
};

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(before: &[u8]) -> crate::RecoverableNativeEditPlan {
    image(before, 30611)
        .plan_recoverable_retained_empty_row_updates("Table1", groups(), &mut budget(), &|| false)
        .unwrap()
}

fn decode(before: &[u8]) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(
        &literal(before, "Table1", &["Score"]),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn empty_original_reconstructs_identity_and_v3_state_outcomes_remain_distinct() {
    let source = joined_update::source();
    for committed in [false, true] {
        let plan = plan(&source.bytes);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16RetainedEmptyRows
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&source.bytes, "Table1", &["Score"])
        );
        assert_eq!(plan.before_bytes(), source.bytes);
        assert_eq!(plan.after_bytes(), source.bytes);
        let plan = plan
            .requalify_with_budgets(
                image(&source.bytes, 30612),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), source.bytes);
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        let prepared = plan
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
        }
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let qualified = decode(&source.bytes)
            .qualify_retained_recovery(checked, CatalogScope::new(30613), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            qualified.state(),
            if committed {
                ResizableState::Committed
            } else {
                ResizableState::Pending
            }
        );
        let outcome = qualified
            .recover_retained(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(target.bytes.get_ref(), &source.bytes);
        let writes = target.writes;
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let outcome = decode(&source.bytes)
            .qualify_retained_recovery(checked, CatalogScope::new(30614), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn old_protocols_and_retagged_nonempty_actions_cannot_replay_zero_match_originals() {
    let source = joined_update::source();
    let mut target = MemoryStorage::new(source.bytes.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&source.bytes)
            .prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    assert!(
        plan(&source.bytes)
            .prepare_update(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal_file,
            &source.bytes,
            &source.bytes,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            decode(&source.bytes),
            CatalogScope::new(30621),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);

    for (magic, profile) in [
        (b"TCNI0017", 17_u16),
        (b"TCNI0018", 18),
        (b"TCNI0019", 19),
        (b"TCNI0020", 20),
        (b"TCNI0021", 21),
        (b"TCNI0022", 22),
        (b"TCNI0023", 23),
    ] {
        let mut wire = literal(&source.bytes, "Table1", &["Score"]);
        wire[..8].copy_from_slice(magic);
        wire[12..14].copy_from_slice(&profile.to_le_bytes());
        let original = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &source.bytes,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            original
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30622),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "profile={profile}"
        );
        assert_eq!(target.writes, 0, "profile={profile}");
    }
}

#[test]
fn forged_after_stale_binding_and_missing_targets_refuse_before_recovery_effects() {
    let source = joined_update::source();
    for offset in [
        3 * 4096 + 3000,
        source.target_definition as usize * 4096 + 3000,
        (source.target_definition as usize + 1) * 4096 + 4000,
    ] {
        let mut forged = source.bytes.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            decode(&source.bytes)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30631),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "offset={offset}"
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &source.bytes);
    }
    for stale_binding in [false, true] {
        let mut original = literal(
            &source.bytes,
            "Table1",
            &[if stale_binding { "Score" } else { "Missing" }],
        );
        if stale_binding {
            original[24] ^= 1;
        }
        let original = NativeUpdateIntent::decode(&original, &mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &source.bytes,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            original
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30632),
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &source.bytes);
    }
}

#[test]
fn multiple_ordinary_empty_targets_bind_each_native_field_and_reject_a_missing_member() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = field_changes::keyless_first_field(&seed, "Keyless");
    let make_groups = |names: &[&str]| {
        names
            .iter()
            .map(|name| {
                NativeFittedIntentGroup::new_retained(
                    name,
                    NativeFittedUpdates::Fixed(Vec::new()),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
            })
            .collect()
    };
    let plan = image(&before, 30641)
        .plan_recoverable_retained_empty_row_updates(
            "Keyless",
            make_groups(&["Id", "Score"]),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), before);
    let wire = literal(&before, "Keyless", &["Id", "Score"]);
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        wire
    );
    let original = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    original
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        plan.prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let qualified = original
        .qualify_retained_recovery(checked, CatalogScope::new(30642), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(qualified.state(), ResizableState::Pending);
    drop(qualified);
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
    assert!(matches!(
        image(&before, 30643).plan_recoverable_retained_empty_row_updates(
            "Keyless",
            make_groups(&["Id", "Missing"]),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::Column
        )))
    ));
}

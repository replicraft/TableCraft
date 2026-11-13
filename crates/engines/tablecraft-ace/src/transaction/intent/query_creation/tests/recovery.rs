//! Whole-image QueryDef creation and the same checked V3 recovery owner.
use super::{Error, Intent, budget, literal};
use crate::{NativeImage, QueryCreationDefinition, RecoverableNativeQueryCreationPlan};
use std::io::Cursor;
use tablecraft_core::values::LegacyDateTime;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::saved_queries::creation as raw, support::io::journal::MemoryStorage,
};

fn date() -> LegacyDateTime {
    LegacyDateTime::from_ole_days(45000.5).unwrap()
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(31910),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(case: &raw::Source, names: [&str; 2]) -> RecoverableNativeQueryCreationPlan {
    let definition =
        QueryCreationDefinition::capture(names[0], names[1], -7, date(), &mut budget(), &|| false)
            .unwrap();
    image(&case.before)
        .plan_recoverable_query_creation(definition, &mut budget(), &|| false)
        .unwrap()
}

fn wire(case: &raw::Source, names: [&str; 2]) -> Vec<u8> {
    literal(&case.before, names, case.native_query_id, -7, date())
}

fn original(case: &raw::Source, names: [&str; 2]) -> Intent {
    Intent::decode(&wire(case, names), &mut budget(), &|| false).unwrap()
}

#[test]
fn actual_creation_original_matches_literal_wire_and_independent_full_after() {
    for names in [
        ["Created", "Answer"],
        ["\u{feff}Created", "\u{feff}Heading"],
    ] {
        let case = raw::source(names[0], names[1], -7, date());
        let captured = plan(&case, names);
        assert_eq!(captured.before_bytes(), case.before);
        assert_eq!(captured.after_bytes(), case.expected);
        assert_eq!(captured.native_query_id(), case.native_query_id);
        assert_eq!(
            captured.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case, names)
        );
        let shared = captured.after_shared_bytes();
        assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
        let rebuilt = captured
            .requalify_with_budgets(image(&case.before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(rebuilt.after_bytes(), case.expected);
        drop(rebuilt);
        assert_eq!(shared.as_ref(), case.expected);
    }
}

#[test]
fn pending_genuine_mixed_committed_and_settled_use_one_checked_v3_owner() {
    let names = ["Created", "Answer"];
    let case = raw::source(names[0], names[1], -7, date());
    for state in 0..3 {
        // NO_CLONE: Each independent journal target owns mutable publication bytes;
        // the raw BEFORE remains the unchanged recovery oracle for all states.
        let mut target = MemoryStorage::new(case.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        let prepared = plan(&case, names)
            .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
            .unwrap();
        if state == 2 {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            if state == 1 {
                let differences: Vec<_> = case
                    .before
                    .iter()
                    .zip(&case.expected)
                    .enumerate()
                    .filter_map(|(offset, (a, b))| (a != b).then_some(offset))
                    .collect();
                assert!(differences.len() > 1);
                let end = differences[differences.len() / 2];
                target.bytes.get_mut()[..end].copy_from_slice(&case.expected[..end]);
                assert_ne!(target.bytes.get_ref(), &case.before);
                assert_ne!(target.bytes.get_ref(), &case.expected);
            }
        }
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        let qualified = original(&case, names)
            .qualify_retained_recovery(checked, CatalogScope::new(31911), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            qualified.state(),
            if state == 2 {
                ResizableState::Committed
            } else {
                ResizableState::Pending
            }
        );
        assert_eq!(
            qualified
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap(),
            if state == 2 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref(),
            if state == 2 {
                &case.expected
            } else {
                &case.before
            }
        );
        let writes = target.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        original(&case, names)
            .qualify_retained_recovery(checked, CatalogScope::new(31912), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn canonical_identity_request_source_and_full_after_tampering_refuse_before_effects() {
    let names = ["Created", "Answer"];
    let case = raw::source(names[0], names[1], -7, date());
    // NO_CLONE: The mutable stale target is independent of the literal source
    // retained for plan construction and the complete unchanged oracle.
    let mut stale = case.before.clone();
    stale[3 * 4096 + 3000] ^= 1;
    // NO_CLONE: The target owns the stale publication image; the oracle remains
    // independently available after the consuming prepare attempt.
    let mut target = MemoryStorage::new(stale.clone());
    let mut file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case, names)
            .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, file.writes), (0, 0));
    assert_eq!(target.bytes.get_ref(), &stale);

    for offset in [24, 56, 60, 64, 80, 87] {
        let mut wrong = wire(&case, names);
        wrong[offset] ^= 1;
        let intent = Intent::decode(&wrong, &mut budget(), &|| false).unwrap();
        // NO_CLONE: The protocol target mutates independently of the raw BEFORE
        // and AFTER supplied as immutable journal qualification oracles.
        let mut target = MemoryStorage::new(case.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut file,
                &case.before,
                &case.expected,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let writes = file.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        assert!(
            intent
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31913),
                    &mut budget(),
                    &|| false,
                )
                .is_err(),
            "wire offset {offset}"
        );
        assert_eq!(target.writes, 0);
        assert_eq!(file.writes, writes);
        assert_eq!(target.bytes.get_ref(), &case.before);
    }

    let new_byte = case
        .before
        .iter()
        .zip(&case.expected)
        .position(|(a, b)| a != b)
        .unwrap();
    for offset in [3 * 4096 + 3000, new_byte] {
        // NO_CLONE: Each forged journal owns a distinct complete AFTER while
        // keeping the independent raw expected image available unchanged.
        let mut forged = case.expected.clone();
        forged[offset] ^= 1;
        // NO_CLONE: This protocol target owns mutable publication state and must
        // remain independent of the complete BEFORE used to bind the journal.
        let mut target = MemoryStorage::new(case.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut file,
                &case.before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let writes = file.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        assert!(
            original(&case, names)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31914),
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(file.writes, writes);
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
}

#[test]
fn cancelled_unpaid_and_legacy_journal_framing_grant_no_creation_effects() {
    let names = ["Created", "Answer"];
    let case = raw::source(names[0], names[1], -7, date());
    for cancelled in [false, true] {
        // NO_CLONE: The mutable target cannot borrow the complete immutable
        // BEFORE needed by journal qualification and no-effects assertions.
        let mut target = MemoryStorage::new(case.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut file,
                &case.before,
                &case.expected,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let writes = file.writes;
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        let mut pool = if cancelled {
            budget()
        } else {
            tablecraft_core::limits::ResourceBudget::new(0, 0, 0)
        };
        let result = original(&case, names).qualify_retained_recovery(
            checked,
            CatalogScope::new(31915),
            &mut pool,
            &|| cancelled,
        );
        if cancelled {
            assert!(matches!(
                result,
                Err(Error::Inspect(crate::InspectError::Cancelled))
            ));
        } else {
            assert!(matches!(
                result,
                Err(Error::Inspect(crate::InspectError::Limit(_)))
            ));
        }
        assert_eq!(target.writes, 0);
        assert_eq!(file.writes, writes);
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
    for v2 in [false, true] {
        // NO_CLONE: Each actual legacy journal uses an independently owned
        // mutable target; neither legacy framing can enter checked V3 creation.
        let mut target = MemoryStorage::new(case.before.clone());
        let mut file = MemoryStorage::new(Vec::new());
        if v2 {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut file,
                    &case.before,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut file,
                    &case.before,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        }
        let writes = file.writes;
        assert!(journal::inspect_v3(&mut file, &mut budget(), &|| false).is_err());
        assert_eq!(target.writes, 0);
        assert_eq!(file.writes, writes);
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
    assert!(
        crate::NativeUpdateIntent::decode(&wire(&case, names), &mut budget(), &|| false).is_err()
    );
}

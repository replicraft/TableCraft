//! Fresh V3 sessions reconstruct independently authored complete INDEX images.
use super::{budget, literal};
use crate::{NativeImage, NativeIndexCreateIntent, SecondaryIndexCreate};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::{self as raw, Population},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn wire(case: &raw::Source) -> Vec<u8> {
    let number = u16::from(case.table == "CompanionValues");
    literal(
        &case.before,
        [case.table, case.column, case.index_name],
        case.target_definition,
        number,
        case.descending,
    )
}

fn intent(case: &raw::Source) -> NativeIndexCreateIntent {
    NativeIndexCreateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

fn plan(case: &raw::Source) -> crate::RecoverableNativeIndexCreatePlan {
    image(&case.before, 30201)
        .plan_recoverable_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn same_bytes(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn index_original_matches_literal_and_requalifies_in_fresh_scopes() {
    for case in [
        raw::source(Population::NullableDuplicate, false),
        raw::protected_companion(true),
    ] {
        let plan = plan(&case);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        same_bytes(plan.before_bytes(), &case.before);
        same_bytes(plan.after_bytes(), &case.expected);
        assert_eq!(plan.table(), case.table);
        assert_eq!(plan.column(), case.column);
        assert_eq!(plan.index_name(), case.index_name);
        assert_eq!(plan.indexed_rows(), case.rows.len());
        let reconstructed = intent(&case)
            .reconstruct(image(&case.before, 30202), &mut budget(), &|| false)
            .unwrap();
        same_bytes(reconstructed.candidate_bytes(), &case.expected);
    }
}

#[test]
fn pending_mixed_committed_and_settled_index_sessions_keep_exact_original_owner() {
    for case in [
        raw::source(Population::NullableDuplicate, true),
        raw::protected_companion(false),
    ] {
        for state in 0..3 {
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan(&case)
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if state == 2 {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                if state == 1 {
                    // A real mixed target: alternating complete old/new pages,
                    // including the longer EOF, never a newly inferred native state.
                    *target.bytes.get_mut() = case.expected.to_vec();
                    for (number, page) in case.before.chunks(4096).enumerate() {
                        if number % 2 == 0 {
                            target.bytes.get_mut()[number * 4096..(number + 1) * 4096]
                                .copy_from_slice(page);
                        }
                    }
                }
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = intent(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30211),
                    &mut budget(),
                    &|| false,
                )
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
            same_bytes(
                target.bytes.get_ref(),
                if state == 2 {
                    &case.expected
                } else {
                    &case.before
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            intent(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30212),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

#[test]
fn stale_target_wrong_original_and_foreign_after_never_grant_recovery_writes() {
    let case = raw::protected_companion(false);
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    same_bytes(target.bytes.get_ref(), &stale);
    for offset in [
        3 * 4096 + 3000,
        case.target_definition as usize * 4096 + 3000,
        (case.target_definition as usize + 1) * 4096 + 4000,
    ] {
        let mut forged = case.expected.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &case.before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            intent(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30221),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "offset={offset}"
        );
        assert_eq!(target.writes, 0);
        same_bytes(target.bytes.get_ref(), &case.before);
    }
    for offset in [14, 24, 64, 66] {
        let mut wrong = wire(&case);
        wrong[offset] ^= 1;
        let original = NativeIndexCreateIntent::decode(&wrong, &mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &case.before,
                &case.expected,
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
                    CatalogScope::new(30222),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "offset={offset}"
        );
        assert_eq!(target.writes, 0);
        same_bytes(target.bytes.get_ref(), &case.before);
    }
}

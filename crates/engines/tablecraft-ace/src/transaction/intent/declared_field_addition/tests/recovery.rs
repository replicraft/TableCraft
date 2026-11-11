//! Cold recovery replays the exact declaration, not SQL or gallery defaults.
use super::{Intent, budget, literal};
use crate::{
    CreateColumn, DeclaredFieldAdditionDefinition, NativeImage,
    RecoverableDeclaredFieldAdditionPlan,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::declared_fields as raw,
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
fn plan(source: &raw::Source) -> RecoverableDeclaredFieldAdditionPlan {
    let definition = DeclaredFieldAdditionDefinition::capture(
        CreateColumn::new("Amount", ScalarType::Currency),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    image(&source.bytes, 30500)
        .plan_recoverable_retained_declared_field_addition(
            "Items",
            definition,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn original(source: &raw::Source) -> Intent {
    Intent::decode(
        &literal(&source.bytes, "Items", "Amount", source.definition),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn declared_plan_moves_original_and_requalifies_with_shared_after_without_policy_invention() {
    let source = raw::source();
    let after = raw::after(&source.bytes, source.definition, "Amount");
    let captured = plan(&source);
    same(captured.before_bytes(), &source.bytes);
    same(captured.after_bytes(), &after);
    assert_eq!(
        captured.intent().encode(&mut budget(), &|| false).unwrap(),
        literal(&source.bytes, "Items", "Amount", source.definition)
    );
    assert_eq!(captured.native_number(), 2);
    assert_eq!(captured.column_id(), 2);
    assert_eq!(captured.column_count(), 2);
    assert!(captured.definition().column().default_literal().is_none());
    let shared = captured.after_shared_bytes();
    assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
    let rebuilt = captured
        .requalify_with_budgets(
            image(&source.bytes, 30501),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(rebuilt.after_bytes(), &after);
    drop(rebuilt);
    same(&shared, &after);
    let mut stale = source.bytes.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    assert!(
        plan(&source)
            .requalify_with_budgets(image(&stale, 30502), &mut budget(), &mut budget(), &|| {
                false
            })
            .is_err()
    );
    assert!(
        plan(&source)
            .requalify_with_budgets(
                image(&source.bytes, 30503),
                &mut budget(),
                &mut tablecraft_core::limits::ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn declared_v3_pending_mixed_committed_and_settled_reconstruct_exact_original() {
    let source = raw::source();
    let after = raw::after(&source.bytes, source.definition, "Amount");
    for state in 0..3 {
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        let prepared = plan(&source)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .unwrap();
        if state == 2 {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            if state == 1 {
                // Only the literal target TDEF differs, making this the actual
                // changed-page prefix of a partially written replacement.
                let base = source.definition as usize * 4096;
                target.bytes.get_mut()[base..base + 100].copy_from_slice(&after[base..base + 100]);
            }
        }
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let qualified = original(&source)
            .qualify_retained_recovery(checked, CatalogScope::new(30510), &mut budget(), &|| false)
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
        same(
            target.bytes.get_ref(),
            if state == 2 { &after } else { &source.bytes },
        );
        let writes = target.writes;
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        original(&source)
            .qualify_retained_recovery(checked, CatalogScope::new(30511), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn declared_stale_binding_wrong_identity_and_forged_after_grant_no_target_writes() {
    let source = raw::source();
    let after = raw::after(&source.bytes, source.definition, "Amount");
    let mut stale = source.bytes.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&source)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
    for offset in [
        3 * 4096 + 3000,
        source.definition as usize * 4096 + 63 + 25 + 23,
        (source.definition as usize + 1) * 4096 + 4000,
    ] {
        let mut forged = after.to_vec();
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
            original(&source)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30520),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "offset {offset}"
        );
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &source.bytes);
    }
    for offset in [24, 56, 60, 64, 66, 68] {
        let mut wrong = literal(&source.bytes, "Items", "Amount", source.definition);
        wrong[offset] ^= 1;
        let original = Intent::decode(&wrong, &mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &after,
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
                    CatalogScope::new(30521),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "offset {offset}"
        );
        assert_eq!(target.writes, 0);
    }
}

#[test]
fn declared_v3_owner_refuses_legacy_protocols_before_recovery_effects() {
    let source = raw::source();
    let after = raw::after(&source.bytes, source.definition, "Amount");
    for version in [1, 2] {
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &source.bytes,
                    &after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut journal_file,
                    &source.bytes,
                    &after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        }
        let writes = journal_file.writes;
        assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
        assert_eq!(target.writes, 0);
        assert_eq!(journal_file.writes, writes);
        same(target.bytes.get_ref(), &source.bytes);
    }
}

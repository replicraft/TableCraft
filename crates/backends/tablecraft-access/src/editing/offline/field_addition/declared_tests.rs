//! Real original13 staging and stateful V3 recovery use independent native AFTER bytes.
use super::*;
use crate::editing::offline::{
    CompletionKind, VerifiedImage, test_support::saved_field::retired_namespace,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{CreateColumn, DeclaredFieldAdditionDefinition};
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::declared_fields as raw;

const SCOPE: CatalogScope = CatalogScope::new(17313);
fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}
fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}
fn plan(before: &[u8]) -> RecoverableDeclaredFieldAdditionPlan {
    let image =
        NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
            .unwrap();
    let definition = DeclaredFieldAdditionDefinition::capture(
        CreateColumn::new("Amount", ScalarType::Currency),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    image
        .plan_recoverable_retained_declared_field_addition(
            "Items",
            definition,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

#[test]
fn declared_currency_stage_apply_is_schema_only_and_matches_independent_complete_image() {
    let source = raw::source();
    let expected = raw::after(&source.bytes, source.definition, "Amount");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("declared.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.bytes).unwrap();
    let staged = open(&path)
        .stage_reviewed_retained_declared_field_addition_with_budgets(
            plan(&source.bytes),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(&fs::read(&path).unwrap(), &source.bytes);
    assert!(!bundle.exists());
    let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    owner.release().unwrap();
    same(&fs::read(&path).unwrap(), &expected);
}

#[test]
fn declared_currency_cold_pending_committed_and_settled_recovery_rebuilds_original13() {
    let source = raw::source();
    let expected = raw::after(&source.bytes, source.definition, "Amount");
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("declared.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let original = plan(&source.bytes);
            let encoded = original.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(encoded.starts_with(b"TCAC0013"));
            let mut owner = open(&path);
            let slot = owner.slot_mut().unwrap();
            let mut pair = OfflineJournalPair::create(
                slot.target.take().unwrap(),
                &bundle,
                encoded,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            {
                let (mut target, mut journal) = pair.resizable_storage().unwrap();
                let prepared = original
                    .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else if retired {
                    prepared.rollback(&mut budget(), &|| false).unwrap();
                }
            }
            if retired {
                pair.retire(if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                })
                .unwrap();
            }
            slot.pair = Some(pair);
            slot.state = SessionState::RetainedJournal;
            owner.release().unwrap();
            let (owner, receipt) = if retired {
                open(&path)
                    .confirm_retired(&retired_namespace(directory.path()), &mut budget(), &|| {
                        false
                    })
                    .unwrap()
            } else {
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::ColumnAdded)
            );
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            if retired {
                assert_eq!(receipt.kind(), CompletionKind::Confirmed);
                assert_eq!(
                    receipt.verified_image(),
                    Some(if committed {
                        VerifiedImage::Replacement
                    } else {
                        VerifiedImage::Original
                    })
                );
            } else {
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
            }
            owner.release().unwrap();
            same(
                &fs::read(&path).unwrap(),
                if committed { &expected } else { &source.bytes },
            );
        }
    }
}

#[test]
fn declared_stage_allocation_and_cancellation_refusals_keep_exclusive_owner_before_namespace() {
    let source = raw::source();
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("declared.accdb");
        fs::write(&path, &source.bytes).unwrap();
        let mut native = ResourceBudget::new(0, 0, 0);
        let mut proof = budget();
        let failure = open(&path)
            .stage_reviewed_retained_declared_field_addition_with_budgets(
                plan(&source.bytes),
                &mut native,
                &mut proof,
                &|| cancelled,
            )
            .err()
            .expect("expected refusal before allocating stage box");
        assert_eq!(failure.phase(), Phase::Planning);
        assert!(failure.pair_state().is_none());
        assert!(failure.current_outcome().is_none());
        if cancelled {
            assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Cancelled)
            ));
        } else {
            assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Limit(_))
            ));
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        same(&fs::read(&path).unwrap(), &source.bytes);
        failure.into_session().release().unwrap();
    }
}

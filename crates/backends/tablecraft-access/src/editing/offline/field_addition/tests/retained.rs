//! Real retained ADD owners distinguish V3 recovery from original V2 schema actions.
use crate::editing::offline::{
    CompletionKind, OfflineWriteSession, Receipt, SchemaChange, SessionState, VerifiedImage,
    test_support::{assert_completion, assert_staged_read_state, saved_field::retired_namespace},
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{NativeImage, RecoverableCurrentFieldAdditionPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields::{
    self, Case, Target,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(1530), &mut budget(), &|| false).unwrap()
}

fn review(before: &[u8], target: Target) -> RecoverableCurrentFieldAdditionPlan {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(1531),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    match target {
        Target::Number => image.plan_recoverable_retained_number_field_addition(
            "Table1",
            target.name(),
            &mut budget(),
            &|| false,
        ),
        Target::ShortText => image.plan_recoverable_retained_short_text_field_addition(
            "Table1",
            target.name(),
            255,
            true,
            &mut budget(),
            &|| false,
        ),
    }
    .unwrap()
}

fn receipt(receipt: &Receipt, added: bool) {
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        added.then_some(SchemaChange::ColumnAdded)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
    assert_completion(receipt);
}

#[test]
fn retained_add_stages_without_namespace_then_publishes_schema_only_receipt() {
    for (case, target) in [
        (Case::FirstNumber, Target::Number),
        (Case::SecondText, Target::ShortText),
    ] {
        let source = fields::source(case);
        let after = fields::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            target,
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("retained.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let staged = open(&path)
            .stage_reviewed_current_field_addition_with_budgets(
                review(&source.bytes, target),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_staged_read_state(&path, &source.bytes);
        assert!(!bundle.exists());
        let (owner, result) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        receipt(&result, true);
        assert_eq!(result.kind(), CompletionKind::Applied);
        assert_eq!(result.outcome(), Some(RecoveryOutcome::Committed));
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
    }
}

#[test]
fn retained_add_cold_and_settled_sessions_preserve_original_v3_action_effects() {
    let source = fields::source(Case::SecondText);
    let after = fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        Target::ShortText,
    );
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let mut owner = open(&path);
            let plan = review(&source.bytes, Target::ShortText);
            let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(encoded.starts_with(b"TCAC0011"));
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
                let prepared = plan
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
            let (owner, result) = if retired {
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
            receipt(&result, committed);
            if retired {
                assert_eq!(result.kind(), CompletionKind::Confirmed);
                assert_eq!(
                    result.verified_image(),
                    Some(if committed {
                        VerifiedImage::Replacement
                    } else {
                        VerifiedImage::Original
                    })
                );
            } else {
                assert_eq!(result.kind(), CompletionKind::Recovered);
                assert_eq!(
                    result.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
            }
            owner.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed { &after } else { &source.bytes }.as_slice()
            );
        }
    }
}

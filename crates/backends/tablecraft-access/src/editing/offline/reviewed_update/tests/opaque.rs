//! Actual SQL and exclusive save owners select opaque rows before complete-row17.
use super::{budget, open};
use crate::editing::{
    offline::{
        CompletionKind, SessionState, VerifiedImage,
        test_support::{
            assert_completion, assert_staged_read_state, saved_field::retired_namespace,
        },
    },
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile, RecoverableNativeEditPlan};
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_query::UpdatePlan;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::opaque_rows as raw;

fn review(before: &[u8]) -> UpdatePlan<RecoverableNativeEditPlan> {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(1640),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query(
        "UPDATE Table1 SET NextNumber=-42 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
        .unwrap()
}

#[test]
fn reviewed_opaque_sql_stage_and_save_publish_exact_image_and_row_counts() {
    for case in raw::CASES {
        let source = raw::source(case);
        let expected = raw::after(&source.bytes, source.definition, source.roots[0], Some(-42));
        let reviewed = review(&source.bytes);
        assert_eq!(reviewed.matched_rows(), 1);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opaque.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
        assert_staged_read_state(&path, &source.bytes);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
    }
}

#[test]
fn opaque_update_cold_and_settled_v3_sessions_keep_original_protocol_and_receipts() {
    let source = raw::source(raw::Case::VariableRetired);
    let expected = raw::after(&source.bytes, source.definition, source.roots[0], Some(-42));
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let plan = review(&source.bytes).into_backend();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16RetainedOpaqueRows
            );
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(wire.starts_with(b"TCNI0018"));
            let mut owner = open(&path);
            let slot = owner.slot_mut().unwrap();
            let mut pair = OfflineJournalPair::create(
                slot.target.take().unwrap(),
                &bundle,
                wire,
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
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(receipt.schema_change(), None);
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
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed { &expected } else { &source.bytes }.as_slice()
            );
        }
    }
}

//! Real retained update journal ownership shared by original carrier families.
use super::{assert_completion, saved_field::retired_namespace};
use crate::editing::offline::{CompletionKind, OfflineWriteSession, SessionState, VerifiedImage};
use std::fs;
use tablecraft_ace::{NativeUpdateProfile, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn open(path: &std::path::Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(173), &mut budget(), &|| false).unwrap()
}

pub(in crate::editing::offline) fn assert_recovery(
    before: &[u8],
    after: &[u8],
    profile: NativeUpdateProfile,
    marker: &[u8; 8],
    make_plan: impl Fn() -> RecoverableNativeEditPlan,
) {
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, before).unwrap();
            let plan = make_plan();
            assert_eq!(plan.intent().profile(), profile);
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert!(wire.starts_with(marker));
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
            let actual = fs::read(&path).unwrap();
            let expected = if committed { after } else { before };
            assert_eq!(actual.len(), expected.len(), "complete image length");
            assert_eq!(
                actual
                    .iter()
                    .zip(expected)
                    .position(|(left, right)| left != right),
                None,
                "first differing image byte"
            );
        }
    }
}

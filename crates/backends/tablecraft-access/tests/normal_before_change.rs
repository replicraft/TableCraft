#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, path::Path};
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState, VerifiedImage};
use tablecraft_ace::{Int32Update, NativeImage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    ExclusiveFile, IoError,
    journal::{
        RecoveryOutcome,
        filesystem::{OfflineJournalPair, OfflineTarget},
    },
};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::canonical_before_change::{Recipe, source};

const ADMITTED: [Recipe; 5] = [
    Recipe::Sequential,
    Recipe::MacroOnlyTarget,
    Recipe::RestoreOriginal,
    Recipe::Empty,
    Recipe::DormantDelete,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(91)
}

fn locked(path: &Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
fn normal_sql_session_commits_ten_literal_before_change_images_under_one_lease() {
    for header in [5, 6] {
        for recipe in ADMITTED {
            let case = source(header, recipe);
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&database, &case.before).unwrap();
            let mut work = budget();
            let owner =
                OfflineWriteSession::open(&database, scope(), &mut work, &|| false).unwrap();
            let sql = if recipe == Recipe::Empty {
                "UPDATE Items SET Score=17 WHERE Id=-1"
            } else {
                "UPDATE Items SET Score=17 WHERE Id=1"
            };
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let staged = owner
                .plan_update(query.query(), &mut work, None, &|| false)
                .unwrap();
            assert_eq!(staged.matched_rows(), case.updates.len());
            assert_eq!(staged.changed_rows(), case.changed_rows);
            assert_eq!(fs::read(&database).unwrap(), case.before);
            assert!(!bundle.exists());
            locked(&database);

            let (owner, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(case.changed_rows));
            assert_eq!(receipt.matched_rows(), Some(case.updates.len()));
            assert_eq!(owner.last_completed(), Some(receipt));
            assert_eq!(owner.state(), SessionState::Ready);
            assert_eq!(fs::read(&database).unwrap(), case.expected.unwrap());
            assert!(!bundle.exists());
            locked(&database);
            owner.release().unwrap();
            ExclusiveFile::open(&database).unwrap().close().unwrap();
        }
    }
}

#[test]
fn supplied_noops_and_late_raise_refuse_before_bundle_creation_without_losing_lease() {
    for header in [5, 6] {
        for (recipe, sql) in [
            (
                Recipe::UserNoopMacroChanges,
                "UPDATE Items SET Score=Score WHERE Id=1",
            ),
            (Recipe::AllNoop, "UPDATE Items SET Score=Score WHERE Id=1"),
            (
                Recipe::LateRaise,
                "UPDATE Items SET Score=Score+2 WHERE Id<=2",
            ),
            (Recipe::AfterUpdate, "UPDATE Items SET Score=17 WHERE Id=1"),
            (Recipe::NamesOnly, "UPDATE Items SET Score=17 WHERE Id=1"),
        ] {
            let case = source(header, recipe);
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("target.accdb");
            fs::write(&database, &case.before).unwrap();
            let mut work = budget();
            let owner =
                OfflineWriteSession::open(&database, scope(), &mut work, &|| false).unwrap();
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let failure = owner
                .plan_update(query.query(), &mut work, None, &|| false)
                .err()
                .expect("native event/context refusal");
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(failure.last_completed(), None);
            assert_eq!(fs::read(&database).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&database);
            failure.into_session().release().unwrap();
        }
    }
}

#[test]
fn original_request_rederivation_recovers_active_and_confirms_retired_ace_pairs() {
    for header in [5, 6] {
        for committed in [false, true] {
            for retired in [false, true] {
                let case = source(header, Recipe::Sequential);
                let root = tempfile::tempdir().unwrap();
                let database = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&database, &case.before).unwrap();
                let mut work = budget();
                let mut target = OfflineTarget::open(&database, &mut work, &|| false).unwrap();
                let image =
                    NativeImage::capture(&mut target, scope(), &mut work, &|| false).unwrap();
                let plan = image
                    .plan_recoverable_int32_updates(
                        "Items",
                        "Score",
                        vec![Int32Update::new(0, 10, 17)],
                        &mut work,
                        &|| false,
                    )
                    .unwrap();
                let intent = plan.intent().encode(&mut work, &|| false).unwrap();
                assert_eq!(&intent[..8], b"TCNI0002");
                let mut pair =
                    OfflineJournalPair::create(target, &bundle, intent, &mut work, &|| false)
                        .unwrap();
                {
                    let (mut target, mut journal) = pair.storage().unwrap();
                    let prepared = plan
                        .prepare(&mut target, &mut journal, &mut work, &|| false)
                        .unwrap();
                    if committed {
                        assert_eq!(prepared.commit(&mut work, &|| false).unwrap(), 1);
                    } else if retired {
                        assert_eq!(
                            prepared.rollback(&mut work, &|| false).unwrap(),
                            RecoveryOutcome::RolledBack,
                        );
                    } else {
                        drop(prepared);
                    }
                }
                let selected_bundle = if retired {
                    pair.retire(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                    .unwrap();
                    // Observe the actual sole directory after retirement;
                    // never reconstruct its generated nonce or rename it.
                    let directories = fs::read_dir(root.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .filter(|path| path.is_dir())
                        .collect::<Vec<_>>();
                    assert_eq!(directories.len(), 1);
                    directories.into_iter().next().unwrap()
                } else {
                    bundle.clone()
                };
                pair.release().unwrap();
                drop(pair);
                let owner =
                    OfflineWriteSession::open(&database, scope(), &mut work, &|| false).unwrap();
                let (owner, receipt) = if retired {
                    owner
                        .confirm_retired(&selected_bundle, &mut work, &|| false)
                        .unwrap()
                } else {
                    owner
                        .recover_update(&selected_bundle, &mut work, &|| false)
                        .unwrap()
                };
                if retired {
                    assert_eq!(
                        receipt.verified_image(),
                        Some(if committed {
                            VerifiedImage::Replacement
                        } else {
                            VerifiedImage::Original
                        })
                    );
                } else {
                    assert_eq!(
                        receipt.outcome(),
                        Some(if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        })
                    );
                }
                let expected = if committed {
                    case.expected.unwrap()
                } else {
                    case.before
                };
                assert_eq!(fs::read(&database).unwrap(), expected);
                assert_eq!(owner.state(), SessionState::Ready);
                locked(&database);
                owner.release().unwrap();
            }
        }
    }
}

//! Real exclusive files and independent full-image index/recovery witnesses.
use super::*;
use crate::editing::offline::{Cause, CompletionKind, SessionState, VerifiedImage};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{NativeImage, SecondaryIndexCreate};
use tablecraft_io::journal::{self, RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}
fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(19105), &mut budget(), &|| false).unwrap()
}
fn reviewed(source: &raw::Source) -> RecoverableNativeIndexCreatePlan {
    NativeImage::capture_retained(
        &mut Cursor::new(&source.before),
        CatalogScope::new(19105),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_retained_long_index_creation(
        source.table,
        &SecondaryIndexCreate::new(source.index_name, source.column, source.descending),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn normal_index_stage_apply_is_schema_only_and_matches_nullable_directional_whole_images() {
    for population in [Population::Empty, Population::NullableDuplicate] {
        for descending in [false, true] {
            let source = raw::source(population, descending);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &source.before).unwrap();
            let staged = open(&path)
                .stage_reviewed_index_creation_with_budgets(
                    reviewed(&source),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                (staged.table(), staged.column(), staged.index_name()),
                (source.table, source.column, source.index_name)
            );
            assert_eq!(staged.indexed_rows(), source.rows.len());
            assert!(!bundle.exists());
            assert_image(&fs::read(&path).unwrap(), &source.before);
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.kind(), CompletionKind::Applied);
            assert_eq!(receipt.schema_change(), Some(SchemaChange::IndexCreated));
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(session.state(), SessionState::Ready);
            session.release().unwrap();
            assert_image(&fs::read(path).unwrap(), &source.expected);
        }
    }
}

#[test]
fn stale_cancelled_and_exhausted_index_reviews_keep_owner_without_creating_a_namespace() {
    let source = raw::source(Population::NullableDuplicate, false);
    for mode in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.accdb");
        let mut actual = source.before.clone();
        if mode == 0 {
            let end = actual.len() - 1;
            actual[end] ^= 1;
        }
        fs::write(&path, &actual).unwrap();
        let mut proof = if mode == 2 {
            ResourceBudget::new(u64::MAX, u64::MAX, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_index_creation_with_budgets(
                reviewed(&source),
                &mut budget(),
                &mut proof,
                &|| mode == 1,
            )
            .err()
            .expect("stale/cancelled/exhausted review cannot stage");
        failure.into_session().release().unwrap();
        assert_image(&fs::read(path).unwrap(), &actual);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

fn retained_pair(path: &Path, bundle: &Path, source: &raw::Source, committed: bool, retired: bool) {
    fs::write(path, &source.before).unwrap();
    let intent = reviewed(source)
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    let mut owner = open(path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        // AFTER is the independent raw tree, never the native candidate's bytes.
        let prepared = journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            &source.before,
            &source.expected,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            prepared.rollback(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
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
}

#[test]
fn normal_index_cold_pending_committed_and_settled_keep_exact_v3_schema_only_receipts() {
    for descending in [false, true] {
        let source = raw::source(Population::NullableDuplicate, descending);
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("source.accdb");
                let bundle = directory.path().join("journal");
                retained_pair(&path, &bundle, &source, committed, retired);
                let (session, receipt) = if retired {
                    let retired_path = fs::read_dir(directory.path())
                        .unwrap()
                        .map(|entry| entry.unwrap().path())
                        .find(|path| {
                            path.file_name()
                                .unwrap()
                                .to_string_lossy()
                                .starts_with(".tablecraft-retired-")
                        })
                        .unwrap();
                    open(&path)
                        .confirm_retired(&retired_path, &mut budget(), &|| false)
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
                assert_eq!(
                    receipt.schema_change(),
                    committed.then_some(SchemaChange::IndexCreated)
                );
                assert_eq!(
                    receipt.verified_image(),
                    retired.then_some(if committed {
                        VerifiedImage::Replacement
                    } else {
                        VerifiedImage::Original
                    })
                );
                session.release().unwrap();
                assert_image(
                    &fs::read(path).unwrap(),
                    if committed {
                        &source.expected
                    } else {
                        &source.before
                    },
                );
            }
        }
    }
}

#[test]
fn index_creation_dispatch_keeps_its_family_and_unknown_versions_terminal() {
    use crate::editing::offline::plan::MutationIntent;

    let source = raw::source(Population::Empty, false);
    let plan = reviewed(&source);
    let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert!(matches!(
        MutationIntent::decode(&encoded, &mut budget(), &|| false),
        Ok(MutationIntent::IndexCreation(_))
    ));
    for raw in [
        b"TCIC0001".as_slice(),
        b"TCIC0003".as_slice(),
        b"TCIC".as_slice(),
    ] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::IndexCreationIntent(_))
        ));
    }
}

#[test]
fn exact_fresh_prefix_is_owned_before_the_frozen_original_family_decoder() {
    use crate::editing::offline::plan::MutationIntent;
    assert!(matches!(
        MutationIntent::decode(b"TCIC0002", &mut budget(), &|| false),
        Err(Cause::FreshIndexCreationIntent(_))
    ));
}

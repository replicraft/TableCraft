use super::*;
use crate::editing::offline::{CompletionKind, VerifiedImage, plan::MutationIntent};
use std::{fs, os::unix::fs::MetadataExt, path::Path};
use tablecraft_ace::{CreateColumn, FirstTableError};
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation,
    canonical_initial_table_creation::{self, Case},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn definition(text: bool) -> FirstTableDefinition {
    let id = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    let mut columns = vec![id];
    if text {
        columns.push(CreateColumn::short_text("Name", 255).unwrap());
    }
    FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap()
}

fn expected(text: bool) -> Vec<u8> {
    let case = if text { Case::ShortText } else { Case::IdOnly };
    canonical_initial_table_creation::source(case).bytes
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(71), &mut budget(), &|| false).unwrap()
}

#[test]
fn first_table_commits_the_same_file_with_schema_receipt_and_no_row_counts() {
    for text in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let bundle = directory.path().join("journal");
        let original = canonical_blank_creation::source();
        fs::write(&path, &original).unwrap();
        let identity = fs::metadata(&path).unwrap();

        let staged = open(&path)
            .plan_first_table_if_original(definition(text), &original, &mut budget(), &|| false)
            .unwrap();
        let columns = if text { 2 } else { 1 };
        assert_eq!(staged.table(), "Items");
        assert_eq!(staged.column_count(), columns);
        assert!(!bundle.exists());
        assert_eq!(fs::read(&path).unwrap(), original);

        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(
            receipt.schema_change(),
            Some(SchemaChange::FirstTableCreated {
                column_count: columns,
            })
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(session.pair_state(), None);
        assert_eq!(fs::read(&path).unwrap(), expected(text));
        let after = fs::metadata(&path).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        assert!(!bundle.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        session.release().unwrap();
    }
}

#[test]
fn first_table_review_mismatch_and_noncanonical_sources_create_no_journal() {
    for stale_review in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let original = canonical_blank_creation::source();
        let mut changed = original.clone();
        *changed.last_mut().unwrap() ^= 1;
        fs::write(&path, if stale_review { &original } else { &changed }).unwrap();

        let result = if stale_review {
            open(&path).plan_first_table_if_original(
                definition(false),
                &changed,
                &mut budget(),
                &|| false,
            )
        } else {
            open(&path).plan_first_table(definition(false), &mut budget(), &|| false)
        };
        let failure = result.err().expect("unqualified source must refuse");
        if stale_review {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        } else {
            assert!(matches!(
                failure.cause(),
                Cause::FirstTablePlanning(FirstTableError::NotCanonicalBlank)
            ));
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(&path).unwrap(),
            if stale_review { original } else { changed }
        );
        failure.into_session().release().unwrap();
    }
}

fn retained_pair(path: &Path, bundle: &Path, committed: bool, retire: bool) -> OfflineWriteSession {
    fs::write(path, canonical_blank_creation::source()).unwrap();
    let mut session = open(path);
    let slot = session.slot_mut().unwrap();
    let image = NativeImage::capture(
        slot.target.as_mut().unwrap(),
        slot.scope,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_first_table(definition(true), &mut budget(), &|| false)
        .unwrap();
    let intent = plan.encode_intent(&mut budget(), &|| false).unwrap();
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
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retire {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
        }
    }
    if retire {
        let outcome = if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        };
        pair.retire(outcome).unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    session
}

#[test]
fn explicit_first_table_recovery_preserves_schema_kind_and_original_rollback() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let bundle = directory.path().join("journal");
        let session = retained_pair(&path, &bundle, committed, false);
        let (session, receipt) = session.recover_pending(&mut budget(), &|| false).unwrap();

        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::FirstTableCreated { column_count: 2 })
        );
        assert_eq!(
            receipt.outcome(),
            Some(if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            })
        );
        assert_eq!(
            fs::read(&path).unwrap(),
            if committed {
                expected(true)
            } else {
                canonical_blank_creation::source()
            }
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        session.release().unwrap();
    }
}

#[test]
fn retired_first_table_confirmation_reports_current_image_and_schema_separately() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let bundle = directory.path().join("journal");
        retained_pair(&path, &bundle, committed, true)
            .release()
            .unwrap();
        let retired = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.is_dir())
            .unwrap();
        let before_confirmation = fs::read(&path).unwrap();
        let (session, receipt) = open(&path)
            .confirm_retired(&retired, &mut budget(), &|| false)
            .unwrap();

        assert_eq!(receipt.kind(), CompletionKind::Confirmed);
        assert_eq!(receipt.outcome(), None);
        assert_eq!(
            receipt.verified_image(),
            Some(if committed {
                VerifiedImage::Replacement
            } else {
                VerifiedImage::Original
            })
        );
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::FirstTableCreated { column_count: 2 })
        );
        assert_eq!(fs::read(&path).unwrap(), before_confirmation);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        session.release().unwrap();
    }
}

#[test]
fn corrupt_first_table_intents_keep_the_selected_decoder_and_never_become_row_actions() {
    for raw in [b"TCNT".as_slice(), b"TCNT0002", b"TCNT0001\0"] {
        let result = MutationIntent::decode(raw, &mut budget(), &|| false);
        assert!(matches!(result, Err(Cause::FirstTableIntent(_))));
    }
}

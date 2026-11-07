//! Explicit restart uses the real reserved sibling and native qualified intent.
use super::terminal;
#[cfg(feature = "ace16-create")]
use crate::SaveSchemaChange;
use crate::{
    SaveCompletion, SaveKnownOutcome, SaveOperation, SaveOutcome, SaveStartFailure, SaveWorker,
    journal_path,
};
use std::fs;
use tablecraft_io::ExclusiveFile;
use tablecraft_testkit::support::access::restart;

#[test]
fn reserved_restart_recovers_pending_or_committed_images_and_keeps_terminal_ownership() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.mdb");
        let journal = journal_path(&source);
        let images = restart::stage(&source, &journal, committed);
        let worker = SaveWorker::new(|| {});
        let operation = worker.start_reserved_recovery(source.clone()).unwrap();
        let refused = worker.start_reserved_recovery(source.clone()).unwrap_err();
        assert_eq!(refused.kind, SaveStartFailure::Busy);
        assert_eq!(refused.into_reviewed().unwrap(), source);
        let report = terminal(&worker);
        assert_eq!(report.id, operation);
        assert_eq!(report.operation, SaveOperation::Recover);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("checked restart failed: {report:?}");
        };
        assert_eq!(receipt.completion, SaveCompletion::Recovered);
        assert_eq!(
            receipt.known,
            if committed {
                SaveKnownOutcome::Committed
            } else {
                SaveKnownOutcome::RolledBack
            }
        );
        assert_eq!(receipt.matched_rows, None);
        assert_eq!(receipt.changed_rows, None);
        assert_eq!(worker.poll(), Some(report));
        assert!(worker.start_reserved_recovery(source.clone()).is_err());
        assert!(worker.acknowledge(operation));
        assert!(!worker.is_busy());
        assert_eq!(
            fs::read(&source).unwrap(),
            if committed {
                images.after
            } else {
                images.before
            }
        );
        assert!(!journal.exists());
        ExclusiveFile::open(&source).unwrap().close().unwrap();
    }
}

#[test]
fn malformed_or_foreign_reserved_journal_retains_failure_until_explicit_release() {
    for foreign in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("selected.mdb");
        let journal = journal_path(&source);
        let expected = if foreign {
            let original = directory.path().join("original.mdb");
            let original_journal = journal_path(&original);
            let images = restart::stage(&original, &original_journal, false);
            fs::write(&source, &images.before).unwrap();
            fs::rename(&original_journal, &journal).unwrap();
            images.before
        } else {
            fs::write(&source, b"do not read as a normal database").unwrap();
            fs::write(&journal, b"malformed reserved entry").unwrap();
            fs::read(&source).unwrap()
        };
        let worker = SaveWorker::new(|| {});
        let operation = worker.start_reserved_recovery(source.clone()).unwrap();
        let report = terminal(&worker);
        let SaveOutcome::Failed(failure) = report.outcome else {
            panic!("unexpected restart result: {report:?}");
        };
        assert_eq!(report.operation, SaveOperation::Recover);
        assert!(failure.can_release);
        assert!(!worker.acknowledge(operation));
        assert!(worker.start_reserved_recovery(source.clone()).is_err());
        let release = worker.release(operation).unwrap();
        let terminal = terminal(&worker);
        assert_eq!(terminal.id, release);
        assert!(matches!(terminal.outcome, SaveOutcome::Released { .. }));
        assert!(worker.acknowledge(release));
        assert_eq!(fs::read(&source).unwrap(), expected);
        assert!(journal.exists());
        ExclusiveFile::open(&source).unwrap().close().unwrap();
    }
}

#[cfg(feature = "ace16-create")]
#[test]
fn reserved_ace16_first_table_restart_recovers_v2_length_and_committed_schema_facts() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.accdb");
        let journal = journal_path(&source);
        let images = restart::stage_first_table(&source, &journal, committed);
        assert_ne!(images.before.len(), images.after.len());
        let worker = SaveWorker::new(|| {});
        let operation = worker.start_reserved_recovery(source.clone()).unwrap();
        let report = terminal(&worker);
        assert_eq!(report.id, operation);
        assert_eq!(report.operation, SaveOperation::Recover);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("native first-table restart failed: {report:?}");
        };
        assert_eq!(receipt.completion, SaveCompletion::Recovered);
        assert_eq!(
            receipt.known,
            if committed {
                SaveKnownOutcome::Committed
            } else {
                SaveKnownOutcome::RolledBack
            }
        );
        assert_eq!(
            receipt.schema_change,
            committed.then_some(SaveSchemaChange::FirstTableCreated { column_count: 2 })
        );
        assert_eq!(receipt.matched_rows, None);
        assert_eq!(receipt.changed_rows, None);
        assert!(worker.acknowledge(operation));
        assert_eq!(
            fs::read(&source).unwrap(),
            if committed {
                images.after
            } else {
                images.before
            }
        );
        assert!(!journal.exists());
        ExclusiveFile::open(&source).unwrap().close().unwrap();
    }
}

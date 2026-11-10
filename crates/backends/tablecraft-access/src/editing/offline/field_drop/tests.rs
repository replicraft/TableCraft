//! Retained real-file schema saves against independent complete DROP hypotheses.
mod recovery;

use super::*;
use crate::editing::offline::{
    CompletionKind, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{FieldDropIntentError, InspectError};
use tablecraft_io::{IoError, SharedFile};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

const SCOPE: CatalogScope = CatalogScope::new(1209);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn review(before: &[u8], column: &str) -> RecoverableFieldDropPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_saved_field_drop("Table1", column, &mut budget(), &|| false)
        .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_receipt(receipt: &Receipt, replacement: bool) {
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        replacement.then_some(SchemaChange::ColumnDropped)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
}

#[test]
fn real_number_and_text_drop_stage_inertly_then_apply_only_column_dropped_receipts() {
    for (case, column) in [
        (raw::SourceCase::NumberHistory, "Score"),
        (raw::SourceCase::MixedTextHistory, "Label"),
    ] {
        let before = raw::source(case);
        let expected = raw::remove_nonprimary(&before, column, raw::Variant::AppendedGroupRemoved);
        let plan = review(&before, column);
        assert_eq!(plan.after_bytes(), expected);
        let identity = plan.identity();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drop.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_field_drop_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.table(), "Table1");
        assert_eq!(staged.column(), column);
        assert_eq!(staged.identity(), identity);
        assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_receipt(&receipt, true);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!bundle.exists());
    }
}

#[test]
fn fresh_source_drift_limits_cancel_and_abandoned_staging_keep_exact_owner_without_journal() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    for case in 0..4 {
        let plan = review(&before, "Score");
        let mut current = before.clone();
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.accdb");
        fs::write(&path, &current).unwrap();
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_field_drop_with_budgets(plan, &mut native, &mut proof, &|| case == 3)
            .err()
            .expect("source/controls must refuse before journal setup");
        match case {
            0 => assert!(matches!(failure.cause(), Cause::SourceChanged)),
            1 | 2 => assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Limit(_))
            )),
            3 => assert!(matches!(
                failure.cause(),
                Cause::Native(InspectError::Cancelled)
            )),
            _ => unreachable!(),
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.pair_state(), None);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("abandon.accdb");
    fs::write(&path, &before).unwrap();
    open(&path)
        .stage_reviewed_field_drop_with_budgets(
            review(&before, "Score"),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .into_session()
        .release()
        .unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn replaced_target_path_is_refused_without_touching_either_file_or_creating_journal() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("target.accdb");
    let moved = directory.path().join("original.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let staged = open(&path)
        .stage_reviewed_field_drop_with_budgets(
            review(&before, "Score"),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    fs::rename(&path, &moved).unwrap();
    let foreign = b"foreign replacement stays exact";
    fs::write(&path, foreign).unwrap();
    let failure = staged
        .apply(&bundle, &mut budget(), &|| false)
        .err()
        .expect("retained target entry identity must match before journal setup");
    assert_eq!(failure.phase(), Phase::Setup);
    assert!(matches!(failure.cause(), Cause::Setup));
    assert!(failure.setup_failure().is_some());
    assert!(!bundle.exists());
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), foreign);
    assert_eq!(fs::read(&moved).unwrap(), before);
}

#[test]
fn closed_drop_family_never_retries_another_action_decoder() {
    for bytes in [b"TCFD".as_slice(), b"TCFD0001", b"TCFD0001\0", b"TCFD0002"] {
        assert!(matches!(
            MutationIntent::decode(bytes, &mut budget(), &|| false),
            Err(Cause::FieldDropIntent(FieldDropIntentError::InvalidRecord))
        ));
    }
    assert!(matches!(
        MutationIntent::decode(b"TCFR0002", &mut budget(), &|| false),
        Err(Cause::FieldRenameIntent(_))
    ));
}

mod retained;

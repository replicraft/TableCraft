//! Actual retained file ownership and independent raw rename observations.
mod recovery;

use super::*;
use crate::editing::offline::{
    CompletionKind, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};
use tablecraft_io::{IoError, SharedFile};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off as raw, support::bytes::hex,
};

const SCOPE: CatalogScope = CatalogScope::new(1094);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn review(before: &[u8], column: &str, new_name: &str) -> RecoverableFieldRenamePlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_same_width_field_rename(
            "Table1",
            column,
            new_name,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_receipt(receipt: &Receipt, changed: bool) {
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        changed.then_some(SchemaChange::ColumnRenamed)
    );
    assert_eq!(receipt.query_change(), None);
    assert_eq!(receipt.database_options_change(), None);
}

#[test]
fn actual_number_and_text_rename_save_match_independent_full_image_pins() {
    for (case, column, new_name, pin) in [
        (
            raw::SourceCase::Number,
            "Score",
            "Total",
            "235a57e709bb65bc6a1d0cc6483c2b204f1d3b78924108ff20e96a6671ba263d",
        ),
        (
            raw::SourceCase::Mixed,
            "Label",
            "Title",
            "7604ae8af88cbcaf1be537e03cfc65c6f8095ed20a8f4c1113359c7d176ddfdc",
        ),
    ] {
        let before = raw::source(case);
        let plan = review(&before, column, new_name);
        let expected = plan.after_bytes().to_vec();
        assert_eq!(hex(&Sha256::digest(&expected)), pin);
        let identity = plan.identity();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rename.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_field_rename_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.table(), "Table1");
        assert_eq!(staged.column(), column);
        assert_eq!(staged.new_name(), new_name);
        assert_eq!(staged.identity(), identity);
        assert!(staged.changed());
        assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_receipt(&receipt, true);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}

#[test]
fn noop_and_case_only_requests_remain_distinct_schema_facts() {
    let before = raw::source(raw::SourceCase::Number);
    for new_name in ["Score", "score"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rename.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let plan = review(&before, "Score", new_name);
        let expected = plan.after_bytes().to_vec();
        let staged = open(&path)
            .stage_reviewed_field_rename_with_budgets(plan, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.changed(), new_name != "Score");
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_receipt(&receipt, new_name != "Score");
        owner.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), expected);
    }
}

#[test]
fn source_drift_cancel_and_independent_pool_refusals_retain_owner_without_journal() {
    let before = raw::source(raw::SourceCase::Number);
    for case in 0..4 {
        let plan = review(&before, "Score", "Total");
        let mut current = before.clone();
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
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
            .stage_reviewed_field_rename_with_budgets(plan, &mut native, &mut proof, &|| case == 3)
            .err()
            .expect("refusal must precede journal setup");
        if case == 0 {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.pair_state(), None);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(path).unwrap(), current);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("abandoned.accdb");
    fs::write(&path, &before).unwrap();
    open(&path)
        .stage_reviewed_field_rename_with_budgets(
            review(&before, "Score", "Total"),
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

#[test]
fn closed_field_family_never_retries_another_intent_decoder() {
    for raw in [b"TCFR".as_slice(), b"TCFR0001", b"TCFR0001\0", b"TCFR0002"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::FieldRenameIntent(_))
        ));
    }
}

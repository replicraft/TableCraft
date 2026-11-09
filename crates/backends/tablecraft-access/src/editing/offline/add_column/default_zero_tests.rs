//! Exact Number declarations through real retained V2 target/journal owners.
use super::{SCOPE, budget, open};
use crate::editing::offline::{
    Cause, CompletionKind, SchemaChange, SessionState, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    CreatedDefaultZeroColumnRequired, NativeImage, RecoverableDefaultZeroAddColumnPlan,
    RecoverableFieldAdditionAfterOffHistoryPlan, RecoverableFieldAdditionAfterOffPlan,
    RecoverableFieldAdditionPlan, RecoverableRepeatedDefaultZeroAddColumnPlan,
};
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

fn review(before: &[u8], table: &str, column: &str) -> RecoverableDefaultZeroAddColumnPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_created_add_default_zero_int32_column(
            table,
            column,
            CreatedDefaultZeroColumnRequired::StoredFalse,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn all_number_declarations_commit_exact_sdk_images_with_schema_only_receipts() {
    for (case, table, column) in [
        (raw::Source::IdOnly, "Table1", "Score"),
        (raw::Source::GuiRequiredZero, "Table1", "Extra"),
        (raw::Source::DefaultOnly, "Items", "Extra"),
    ] {
        let before = raw::source(case);
        let after = raw::append(&before, case, column);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("number.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();

        let staged = open(&path)
            .stage_reviewed_default_zero_add_column_with_budgets(
                review(&before, table, column),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!((staged.table(), staged.column()), (table, column));
        assert!(!bundle.exists());
        assert_staged_read_state(&path, &before);

        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_completion(&receipt);
        assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        owner.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), after);
    }
}

fn retained(path: &Path, bundle: &Path, committed: bool, retired: bool) -> Vec<u8> {
    let before = raw::source(raw::Source::IdOnly);
    let plan = review(&before, "Table1", "Score");
    retained_plan(path, bundle, RetainedPlan::First(plan), committed, retired)
}

enum RetainedPlan {
    First(RecoverableDefaultZeroAddColumnPlan),
    Repeated(RecoverableRepeatedDefaultZeroAddColumnPlan),
    Typed(RecoverableFieldAdditionPlan),
    AfterOff(RecoverableFieldAdditionAfterOffPlan),
    AfterOffHistory(RecoverableFieldAdditionAfterOffHistoryPlan),
}

fn retained_plan(
    path: &Path,
    bundle: &Path,
    plan: RetainedPlan,
    committed: bool,
    retired: bool,
) -> Vec<u8> {
    let (before, after, intent) = match &plan {
        RetainedPlan::First(plan) => (
            plan.before_bytes().to_vec(),
            plan.after_bytes().to_vec(),
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
        ),
        RetainedPlan::Typed(plan) => (
            plan.before_bytes().to_vec(),
            plan.after_bytes().to_vec(),
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
        ),
        RetainedPlan::AfterOff(plan) => (
            plan.before_bytes().to_vec(),
            plan.after_bytes().to_vec(),
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
        ),
        RetainedPlan::AfterOffHistory(plan) => (
            plan.before_bytes().to_vec(),
            plan.after_bytes().to_vec(),
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
        ),
        RetainedPlan::Repeated(plan) => (
            plan.before_bytes().to_vec(),
            plan.after_bytes().to_vec(),
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
        ),
    };

    let framing: &[u8] = match &plan {
        RetainedPlan::First(_) => b"TCAC0003",
        RetainedPlan::Repeated(_) => b"TCAC0004",
        RetainedPlan::Typed(_) => b"TCAC0005",
        RetainedPlan::AfterOff(_) => b"TCAC0006",
        RetainedPlan::AfterOffHistory(_) => b"TCAC0007",
    };
    assert!(intent.starts_with(framing));

    fs::write(path, &before).unwrap();
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
        let prepared = match plan {
            RetainedPlan::First(plan) => {
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            }
            RetainedPlan::Repeated(plan) => {
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            }
            RetainedPlan::Typed(plan) => {
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            }
            RetainedPlan::AfterOff(plan) => {
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            }
            RetainedPlan::AfterOffHistory(plan) => {
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
            }
        }
        .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else if retired {
            assert_eq!(
                prepared.rollback(&mut budget(), &|| false).unwrap(),
                RecoveryOutcome::RolledBack
            );
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

    if committed { after } else { before }
}

#[test]
fn cold_and_settled_number_recovery_use_exact_v2_declaration_and_original_length() {
    for retired in [false, true] {
        for committed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            let expected = retained(&path, &bundle, committed, retired);
            let (owner, receipt) = if retired {
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
                receipt.kind(),
                if retired {
                    CompletionKind::Confirmed
                } else {
                    CompletionKind::Recovered
                }
            );
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::ColumnAdded)
            );
            assert_eq!(
                receipt.verified_image(),
                retired.then_some(if committed {
                    VerifiedImage::Replacement
                } else {
                    VerifiedImage::Original
                })
            );
            assert_eq!(
                receipt.outcome(),
                (!retired).then_some(if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                })
            );
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            owner.release().unwrap();
            assert_eq!(fs::read(&path).unwrap(), expected);
        }
    }
}

#[test]
fn malformed_version_three_never_retries_a_frozen_add_column_decoder() {
    for bytes in [b"TCAC0003".as_slice(), b"TCAC0003\0"] {
        assert!(matches!(
            MutationIntent::decode(bytes, &mut budget(), &|| false),
            Err(Cause::DefaultZeroAddColumnIntent(_))
        ));
    }
    assert!(matches!(
        MutationIntent::decode(b"TCAC9999", &mut budget(), &|| false),
        Err(Cause::AddColumnIntent(_))
    ));
}

#[path = "default_zero_tests/failures.rs"]
mod failures;

#[path = "default_zero_tests/repeated.rs"]
mod repeated;

#[path = "default_zero_tests/field_addition.rs"]
mod field_addition;

#[path = "default_zero_tests/after_off.rs"]
mod after_off;

#[path = "default_zero_tests/after_off_history.rs"]
mod after_off_history;

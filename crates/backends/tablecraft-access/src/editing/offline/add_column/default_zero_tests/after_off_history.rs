//! TCAC7 shares the real offline lifecycle while retaining its exact declaration.
use super::{RetainedPlan, SCOPE, budget, open, retained_plan};
use crate::editing::offline::{
    CompletionKind, SchemaChange, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{
    FieldAdditionTarget, NativeImage, RecoverableFieldAdditionAfterOffHistoryPlan,
};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::{
    self as raw, history as sources,
};

fn text(units: usize) -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: units,
        unicode_compression: true,
    }
}

fn review(
    before: &[u8],
    column: &str,
    target: FieldAdditionTarget,
) -> RecoverableFieldAdditionAfterOffHistoryPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_created_field_addition_after_off_history(
            "Table1",
            column,
            target,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn all_four_after_off_saves_commit_complete_independent_images_with_schema_only_receipts() {
    for case in [
        sources::SourceCase::NumberThenNumber,
        sources::SourceCase::MixedThenText,
    ] {
        let before = sources::source(case);
        for (column, target, after) in [
            (
                "SecondNumber",
                FieldAdditionTarget::NumberLongZero,
                raw::append_number(&before, "SecondNumber"),
            ),
            (
                "SecondLabel",
                text(255),
                raw::append_text(&before, "SecondLabel"),
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("field.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let staged = open(&path)
                .stage_reviewed_field_addition_after_off_history_with_budgets(
                    review(&before, column, target),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!((staged.table(), staged.column()), ("Table1", column));
            assert!(!bundle.exists());
            assert_staged_read_state(&path, &before);
            let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_completion(&receipt);
            assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(receipt.query_change(), None);
            assert_eq!(receipt.database_options_change(), None);
            owner.release().unwrap();
            assert_eq!(fs::read(path).unwrap(), after);
        }
    }
}

#[test]
fn cold_and_settled_after_off_recovery_reconstruct_exact_v2_declaration() {
    for retired in [false, true] {
        for committed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recovery.accdb");
            let bundle = directory.path().join("journal");
            let before = sources::source(sources::SourceCase::NumberThenNumber);
            let expected = retained_plan(
                &path,
                &bundle,
                RetainedPlan::AfterOffHistory(review(&before, "SecondLabel", text(255))),
                committed,
                retired,
            );
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
                receipt.verified_image(),
                retired.then_some(if committed {
                    VerifiedImage::Replacement
                } else {
                    VerifiedImage::Original
                })
            );
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::ColumnAdded)
            );
            assert_eq!(
                receipt.kind(),
                if retired {
                    CompletionKind::Confirmed
                } else {
                    CompletionKind::Recovered
                }
            );
            owner.release().unwrap();
            assert_eq!(fs::read(path).unwrap(), expected);
        }
    }
}

#[test]
fn closed_v7_dispatch_never_retries_a_legacy_column_decoder() {
    let before = sources::source(sources::SourceCase::NumberThenNumber);
    let wire = review(&before, "SecondLabel", text(255))
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert!(wire.starts_with(b"TCAC0007"));
    assert!(matches!(
        MutationIntent::decode(&wire, &mut budget(), &|| false),
        Ok(MutationIntent::FieldAdditionAfterOffHistory(_))
    ));
    let mut corrupt = wire;
    corrupt.truncate(8);
    assert!(matches!(
        MutationIntent::decode(&corrupt, &mut budget(), &|| false),
        Err(crate::editing::offline::Cause::FieldAdditionAfterOffHistoryIntent(_))
    ));
}

#[test]
fn fresh_staging_rechecks_original_bytes_before_any_journal_setup() {
    let before = sources::source(sources::SourceCase::NumberThenNumber);
    let reviewed = review(&before, "SecondLabel", text(255));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stale.accdb");
    let bundle = directory.path().join("journal");
    let mut changed = before.clone();
    changed[4095] ^= 1;
    fs::write(&path, &changed).unwrap();
    let failure = match open(&path).stage_reviewed_field_addition_after_off_history_with_budgets(
        reviewed,
        &mut budget(),
        &mut budget(),
        &|| false,
    ) {
        Ok(_) => panic!("changed BEFORE cannot reuse a declaration-bound review"),
        Err(failure) => failure,
    };
    assert!(!bundle.exists());
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), changed);
}

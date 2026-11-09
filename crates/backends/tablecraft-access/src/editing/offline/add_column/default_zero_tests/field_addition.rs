//! TCAC5 shares the real offline lifecycle while retaining its exact declaration.
use super::{RetainedPlan, SCOPE, budget, open, retained_plan};
use crate::editing::offline::{
    SchemaChange, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state},
};
use std::{fs, io::Cursor};
use tablecraft_ace::{FieldAdditionTarget, NativeImage, RecoverableFieldAdditionPlan};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed as raw;

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
) -> RecoverableFieldAdditionPlan {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_created_field_addition("Table1", column, target, &mut budget(), &|| false)
        .unwrap()
}

#[test]
fn text_width_compression_and_number_after_text_commit_exact_independent_images() {
    let first = raw::source(raw::SourceCase::IdOnly);
    let label = raw::append_text(&first, "Label", 255, true);
    for (before, column, target, after) in [
        (first.clone(), "Label", text(255), label.clone()),
        (
            first.clone(),
            "Label",
            text(1),
            raw::append_text(&first, "Label", 1, true),
        ),
        (
            label.clone(),
            "AfterLabel",
            FieldAdditionTarget::NumberLongZero,
            raw::append_number(&label, "AfterLabel"),
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("field.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let checked = review(&before, column, target);
        assert_eq!(checked.target(), target);
        let staged = open(&path)
            .stage_reviewed_field_addition_with_budgets(
                checked,
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

#[test]
fn cold_and_settled_typed_field_recovery_reconstruct_exact_v2_declaration() {
    for retired in [false, true] {
        for committed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recovery.accdb");
            let bundle = directory.path().join("journal");
            let before = raw::source(raw::SourceCase::IdOnly);
            let expected = retained_plan(
                &path,
                &bundle,
                RetainedPlan::Typed(review(&before, "Label", text(255))),
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
                if committed {
                    VerifiedImage::Replacement
                } else {
                    VerifiedImage::Original
                }
            );
            assert_eq!(receipt.schema_change(), Some(SchemaChange::ColumnAdded));
            owner.release().unwrap();
            assert_eq!(fs::read(path).unwrap(), expected);
        }
    }
}

#[test]
fn closed_v5_dispatch_never_retries_a_legacy_column_decoder() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let wire = review(&before, "Label", text(255))
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert!(wire.starts_with(b"TCAC0005"));
    assert!(matches!(
        MutationIntent::decode(&wire, &mut budget(), &|| false),
        Ok(MutationIntent::FieldAddition(_))
    ));
    let mut corrupt = wire;
    corrupt.truncate(8);
    assert!(matches!(
        MutationIntent::decode(&corrupt, &mut budget(), &|| false),
        Err(crate::editing::offline::Cause::FieldAdditionIntent(_))
    ));
}

#[test]
fn fresh_staging_rechecks_original_bytes_before_any_journal_setup() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let reviewed = review(&before, "Label", text(255));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stale.accdb");
    let bundle = directory.path().join("journal");
    let mut changed = before.clone();
    changed[4095] ^= 1;
    fs::write(&path, &changed).unwrap();
    let failure = match open(&path).stage_reviewed_field_addition_with_budgets(
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

//! Exclusive-source requalification and one atomic schema-plus-row receipt.
use super::*;
use crate::editing::offline::{CompletionKind, SchemaChange, SessionState};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{CreateColumn, FirstTableDefinition, NativeImage};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(19101), &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8]) -> PopulatedAddTablePlan {
    plan_rows(before, 900)
}

fn plan_rows(before: &[u8], count: i32) -> PopulatedAddTablePlan {
    let image = NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(19101),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let definition = FirstTableDefinition::capture_manual_native(
        "Archive",
        &[CreateColumn::new("CopiedScore", ScalarType::Int32)],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let values = (0..count)
        .map(|row| {
            vec![if row % 17 == 0 {
                Value::Null
            } else {
                Value::Int32(row)
            }]
        })
        .collect();
    image
        .plan_populated_add_table(definition, values, &mut budget(), &|| false)
        .unwrap()
}

fn expected(before: &[u8]) -> Vec<u8> {
    let literals = (0..900)
        .map(|row| (row % 17 != 0).then_some(row))
        .collect::<Vec<_>>();
    populated::numbers(before, "Archive", &literals)
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn populated_stage_applies_one_full_image_with_schema_and_inserted_count() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let expected = expected(&before);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let staged = open(&path)
        .stage_reviewed_populated_add_table_with_budgets(
            plan(&before),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        (
            staged.table(),
            staged.column_count(),
            staged.inserted_rows()
        ),
        ("Archive", 1, 900)
    );
    assert!(!bundle.exists());
    assert_image(&fs::read(&path).unwrap(), &before);
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, Some(900))
    );
    assert_eq!(
        receipt.schema_change(),
        Some(SchemaChange::TableAdded { column_count: 1 })
    );
    assert_eq!(session.state(), SessionState::Ready);
    session.release().unwrap();
    assert_image(&fs::read(&path).unwrap(), &expected);
    let table = tablecraft_ace::read_table(
        &mut Cursor::new(fs::read(&path).unwrap()),
        CatalogScope::new(19102),
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), 900);
    assert_eq!(table.rows()[1], [Value::Int32(1)]);
    assert_eq!(table.rows()[0], [Value::Null]);
}

#[test]
fn populated_staging_rejects_stale_original_cancel_and_exhausted_proof_without_namespace() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    for mode in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.accdb");
        let mut actual = before.clone();
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
            .stage_reviewed_populated_add_table_with_budgets(
                plan(&before),
                &mut budget(),
                &mut proof,
                &|| mode == 1,
            )
            .err()
            .expect("stale/cancelled/exhausted candidate cannot stage");
        assert!(matches!(
            failure.cause(),
            Cause::Qualification(_) | Cause::Native(_)
        ));
        failure.into_session().release().unwrap();
        assert_image(&fs::read(&path).unwrap(), &actual);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    after: &[u8],
    committed: bool,
    retired: bool,
) {
    use tablecraft_io::journal::{self, RecoveryOutcome, filesystem::OfflineJournalPair};
    fs::write(path, before).unwrap();
    let mut owner = open(path);
    let intent = plan(before)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create_with_intent_limit(
        slot.target.take().unwrap(),
        bundle,
        intent,
        tablecraft_ace::NativePopulatedAddTableIntent::MAX_ENCODED_BYTES as u64,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        // The journal image comes from the independent raw builder, not the plan.
        let prepared = journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            before,
            after,
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
fn populated_cold_v3_pending_committed_and_settled_keep_honest_schema_only_recovery_receipts() {
    use crate::editing::offline::VerifiedImage;
    use tablecraft_io::journal::RecoveryOutcome;
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let expected = expected(&before);
    for committed in [false, true] {
        for retired in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("source.accdb");
            let bundle = directory.path().join("journal");
            retained_pair(&path, &bundle, &before, &expected, committed, retired);
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
                committed.then_some(SchemaChange::TableAdded { column_count: 1 })
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
                if retired {
                    None
                } else {
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                }
            );
            session.release().unwrap();
            assert_image(
                &fs::read(&path).unwrap(),
                if committed { &expected } else { &before },
            );
        }
    }
}

#[test]
fn zero_selected_rows_still_publish_the_declared_table_atomically() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let expected = populated::numbers(&before, "Archive", &[]);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.accdb");
    fs::write(&path, &before).unwrap();
    let staged = open(&path)
        .stage_reviewed_populated_add_table_with_budgets(
            plan_rows(&before, 0),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(staged.inserted_rows(), 0);
    let (session, receipt) = staged
        .apply(&directory.path().join("journal"), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, Some(0))
    );
    assert_eq!(
        receipt.schema_change(),
        Some(SchemaChange::TableAdded { column_count: 1 })
    );
    session.release().unwrap();
    assert_image(&fs::read(path).unwrap(), &expected);
}

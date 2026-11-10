//! Real-file AddTable composition retains original objects and the existing save owner.
mod native;
mod repeated;

use super::*;
use crate::editing::offline::{
    CompletionKind, SessionState,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state, assert_terminal_entries},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{AddTableError, CreateColumn, NativeImage, read_table};
use tablecraft_core::values::Value;
use tablecraft_db::{InsertValue, OmittingInsertableTable};
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{canonical_add_table, canonical_repeated_add_table};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn definition(name: &str) -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 20).unwrap(),
    ];
    FirstTableDefinition::capture(name, &columns, &mut budget(), &|| false).unwrap()
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(82), &mut budget(), &|| false).unwrap()
}

#[test]
fn append_save_preserves_same_file_and_all_original_objects_then_accepts_first_insert() {
    let source = canonical_add_table::source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.before).unwrap();
    #[cfg(unix)]
    let identity = fs::metadata(&path).unwrap();
    let staged = open(&path)
        .plan_add_table_if_original(definition("More"), &source.before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(staged.table(), "More");
    assert_eq!(staged.column_count(), 2);
    assert!(!bundle.exists());
    assert_staged_read_state(&path, &source.before);
    let (mut session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_eq!(
        receipt.schema_change(),
        Some(SchemaChange::TableAdded { column_count: 2 })
    );
    assert_eq!(session.state(), SessionState::Ready);
    assert_eq!(session.last_completed(), Some(receipt));
    assert_completion(&receipt);
    let slot = session.slot_mut().unwrap();
    let image = NativeImage::capture(
        slot.target.as_mut().unwrap(),
        slot.scope,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(image.as_bytes(), &source.after);
    let old = read_table(
        &mut image.reader(),
        slot.scope,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(old.rows(), std::slice::from_ref(&source.original_row));
    let new = read_table(
        &mut image.reader(),
        slot.scope,
        "More",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(new.rows().is_empty());
    let staged = session
        .insert_table_if_original("More", &source.after, &mut budget(), &|| false)
        .unwrap()
        .plan_insert_with_omissions(
            vec![
                InsertValue::Omitted,
                InsertValue::Supplied(Value::Text("hello".into())),
            ],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(staged.inserted_rows(), 1);
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.changed_rows(), Some(1));
    assert_eq!(receipt.schema_change(), None);
    assert_completion(&receipt);
    session.release().unwrap();
    let final_bytes = fs::read(&path).unwrap();
    assert_eq!(
        &final_bytes[7 * 4096..10 * 4096],
        &source.before[7 * 4096..10 * 4096]
    );
    for (table, rows) in [
        ("Items", vec![source.original_row]),
        (
            "More",
            vec![vec![Value::Int32(1), Value::Text("hello".into())]],
        ),
    ] {
        let actual = read_table(
            &mut Cursor::new(&final_bytes),
            CatalogScope::new(83),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), rows);
    }
    #[cfg(unix)]
    {
        let after = fs::metadata(&path).unwrap();
        assert_eq!((identity.dev(), identity.ino()), (after.dev(), after.ino()));
    }
    assert!(!bundle.exists());
    assert_terminal_entries(directory.path(), 2);
}

#[test]
fn duplicate_names_and_stale_original_refuse_without_journal_or_target_changes() {
    for stale in [false, true] {
        let source = canonical_add_table::source();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.accdb");
        fs::write(&path, &source.before).unwrap();
        let result = if stale {
            let mut wrong = source.before.clone();
            wrong[7 * 4096 + 3500] ^= 1;
            open(&path).plan_add_table_if_original(
                definition("More"),
                &wrong,
                &mut budget(),
                &|| false,
            )
        } else {
            open(&path).plan_add_table(definition("Items"), &mut budget(), &|| false)
        };
        let failure = result.err().expect("stale or duplicate schema must refuse");
        if stale {
            assert!(matches!(failure.cause(), Cause::SourceChanged));
        } else {
            assert!(matches!(
                failure.cause(),
                Cause::AddTablePlanning(AddTableError::DuplicateName)
            ));
        }
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.pair_state(), None);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), source.before);
        assert_terminal_entries(directory.path(), 0);
    }
}

fn retained_pair(
    path: &Path,
    bundle: &Path,
    committed: bool,
    before: &[u8],
    table: &str,
    magic: &[u8; 8],
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let mut session = open(path);
    let slot = session.slot_mut().unwrap();
    let plan = NativeImage::capture(
        slot.target.as_mut().unwrap(),
        slot.scope,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_add_table(definition(table), &mut budget(), &|| false)
    .unwrap();
    let raw = plan.encode_intent(&mut budget(), &|| false).unwrap();
    assert_eq!(&raw[..8], magic);
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        bundle,
        raw,
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
        }
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    session
}

#[test]
fn retained_and_cold_tcat_recovery_preserve_table_added_receipt_and_exact_original() {
    let first = canonical_add_table::source();
    let repeated = canonical_repeated_add_table::source();
    for (before, after, table, magic) in [
        (&first.before, &first.after, "More", b"TCAT0001"),
        (&repeated.before, &repeated.after, "Third", b"TCAT0002"),
    ] {
        for committed in [false, true] {
            for cold in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("existing.accdb");
                let bundle = directory.path().join("journal");
                let owner = retained_pair(&path, &bundle, committed, before, table, magic);
                let (session, receipt) = if cold {
                    owner.release().unwrap();
                    open(&path)
                        .recover_update(&bundle, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    owner.recover_pending(&mut budget(), &|| false).unwrap()
                };
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
                assert_eq!(
                    (receipt.matched_rows(), receipt.changed_rows()),
                    (None, None)
                );
                assert_eq!(
                    receipt.schema_change(),
                    committed.then_some(SchemaChange::TableAdded { column_count: 2 })
                );
                assert_eq!(session.state(), SessionState::Ready);
                assert_completion(&receipt);
                session.release().unwrap();
                assert_eq!(
                    fs::read(&path).unwrap(),
                    if committed { after } else { before }.as_slice()
                );
                assert_terminal_entries(directory.path(), 1);
            }
        }
    }
}

#[test]
fn tcat_namespace_failures_keep_add_table_decoder_and_never_fall_back_to_tcnt_or_rows() {
    for raw in [
        b"TCAT".as_slice(),
        b"TCAT0001",
        b"TCAT0001\0",
        b"TCAT0002",
        b"TCAT0003",
    ] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::AddTableIntent(_))
        ));
    }
    for raw in [b"TCNT".as_slice(), b"TCNT0004", b"TCNT0001\0"] {
        assert!(matches!(
            MutationIntent::decode(raw, &mut budget(), &|| false),
            Err(Cause::FirstTableIntent(_))
        ));
    }
}

use super::*;
use crate::editing::offline::{
    CompletionKind, SessionState, VerifiedImage,
    plan::MutationIntent,
    test_support::{assert_completion, assert_staged_read_state, assert_terminal_entries},
};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{
    CreateColumn, FirstTableError, NativeImage, PropertyGroupKind, inspect_table_properties,
    read_table,
};
use tablecraft_core::values::{Currency, Value};
use tablecraft_db::{InsertValue, OmittingInsertableTable};
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
        #[cfg(unix)]
        let identity = fs::metadata(&path).unwrap();

        let staged = open(&path)
            .plan_first_table_if_original(definition(text), &original, &mut budget(), &|| false)
            .unwrap();
        let columns = if text { 2 } else { 1 };
        assert_eq!(staged.table(), "Items");
        assert_eq!(staged.column_count(), columns);
        assert!(!bundle.exists());
        assert_eq!(staged.plan.before_bytes(), original);
        assert_staged_read_state(&path, &original);

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
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected(text));
        #[cfg(unix)]
        {
            let after = fs::metadata(&path).unwrap();
            assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        }
        assert!(!bundle.exists());
        assert_terminal_entries(directory.path(), 1);
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
        failure.into_session().release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            if stale_review { original } else { changed }
        );
    }
}

fn retained_pair(path: &Path, bundle: &Path, committed: bool, retire: bool) -> OfflineWriteSession {
    retained_pair_with_definition(path, bundle, committed, retire, definition(true))
}

fn retained_pair_with_definition(
    path: &Path,
    bundle: &Path,
    committed: bool,
    retire: bool,
    definition: FirstTableDefinition,
) -> OfflineWriteSession {
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
        .plan_first_table(definition, &mut budget(), &|| false)
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
        assert_eq!(session.state(), SessionState::Ready);
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            if committed {
                expected(true)
            } else {
                canonical_blank_creation::source()
            }
        );
        assert_terminal_entries(directory.path(), 1);
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
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
            })
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
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), before_confirmation);
        assert_terminal_entries(directory.path(), 1);
    }
}

#[test]
fn corrupt_first_table_intents_keep_the_selected_decoder_and_never_become_row_actions() {
    for raw in [b"TCNT".as_slice(), b"TCNT0003", b"TCNT0001\0"] {
        let result = MutationIntent::decode(raw, &mut budget(), &|| false);
        assert!(matches!(result, Err(Cause::FirstTableIntent(_))));
    }
}

fn default_definition() -> FirstTableDefinition {
    let zero = Value::Int32(0);
    let large_zero = Value::Int64(0);
    let currency_zero = Value::Currency(Currency::from_scaled(0));
    let no = Value::Boolean(false);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Number", ScalarType::Int32)
            .with_default_literal(&zero)
            .unwrap(),
        CreateColumn::new("LargeNumber", ScalarType::Int64)
            .with_default_literal(&large_zero)
            .unwrap(),
        CreateColumn::new("Money", ScalarType::Currency)
            .with_default_literal(&currency_zero)
            .unwrap(),
        CreateColumn::new("Flag", ScalarType::Boolean)
            .with_default_literal(&no)
            .unwrap(),
        CreateColumn::new("When", ScalarType::DateTime),
    ];
    FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap()
}

fn assert_default_metadata(bytes: &[u8]) {
    let properties = inspect_table_properties(
        &mut Cursor::new(bytes),
        CatalogScope::new(72),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (field, expression) in [
        ("Number", "0"),
        ("LargeNumber", "0"),
        ("Money", "0.0000"),
        ("Flag", "False"),
    ] {
        let property = properties
            .group(PropertyGroupKind::Column, field)
            .unwrap()
            .get("DefaultValue")
            .unwrap();
        assert_eq!(property.native_type(), 12);
        assert_eq!(property.text().unwrap(), expression);
    }
    for field in ["Id", "When"] {
        assert!(
            properties
                .group(PropertyGroupKind::Column, field)
                .and_then(|group| group.get("DefaultValue"))
                .is_none()
        );
    }
}

#[test]
fn first_table_apply_preserves_typed_constant_defaults_without_inventing_a_date_default() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("new.accdb");
    let bundle = directory.path().join("journal");
    let original = canonical_blank_creation::source();
    fs::write(&path, &original).unwrap();
    let staged = open(&path)
        .plan_first_table_if_original(default_definition(), &original, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(staged.plan.before_bytes(), original);
    assert_staged_read_state(&path, &original);
    assert!(!bundle.exists());
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(receipt.matched_rows(), None);
    assert_eq!(receipt.changed_rows(), None);
    assert_eq!(
        receipt.schema_change(),
        Some(SchemaChange::FirstTableCreated { column_count: 6 })
    );
    assert_completion(&receipt);
    session.release().unwrap();
    assert_default_metadata(&fs::read(&path).unwrap());
}

#[test]
fn default_bearing_first_table_intent_recovers_pending_and_committed_images() {
    for committed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let bundle = directory.path().join("journal");
        let session =
            retained_pair_with_definition(&path, &bundle, committed, false, default_definition());
        let (session, receipt) = session.recover_pending(&mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), None);
        assert_eq!(
            receipt.schema_change(),
            committed.then_some(SchemaChange::FirstTableCreated { column_count: 6 })
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert_completion(&receipt);
        session.release().unwrap();
        let bytes = fs::read(&path).unwrap();
        if committed {
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_default_metadata(&bytes);
        } else {
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::RolledBack));
            assert_eq!(bytes, canonical_blank_creation::source());
        }
        assert!(!bundle.exists());
        assert_terminal_entries(directory.path(), 1);
    }
}

#[test]
fn saved_initial_defaults_compose_with_generated_insert_and_preserve_original_presence() {
    for omit_companions in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("new.accdb");
        let bundle = directory.path().join("journal");
        let original = canonical_blank_creation::source();
        fs::write(&path, &original).unwrap();
        let staged = open(&path)
            .plan_first_table_if_original(default_definition(), &original, &mut budget(), &|| false)
            .unwrap();
        let (mut session, _) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let slot = session.slot_mut().unwrap();
        let before_insert = NativeImage::capture(
            slot.target.as_mut().unwrap(),
            slot.scope,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .as_bytes()
        .to_vec();
        let supplied = [
            Value::Int32(7),
            Value::Int64(8),
            Value::Currency(Currency::from_scaled(12345)),
            Value::Boolean(true),
        ];
        let mut input = vec![InsertValue::Omitted];
        input.extend(supplied.iter().map(|value| {
            if omit_companions {
                InsertValue::Omitted
            } else {
                InsertValue::Supplied(value.clone())
            }
        }));
        input.push(InsertValue::Supplied(Value::Null));
        let staged = session
            .insert_table_if_original("Items", &before_insert, &mut budget(), &|| false)
            .unwrap()
            .plan_insert_with_omissions(input, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(staged.inserted_rows(), 1);
        assert_staged_read_state(&path, &before_insert);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(receipt.matched_rows(), None);
        assert_eq!(receipt.changed_rows(), Some(1));
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        session.release().unwrap();
        assert_terminal_entries(directory.path(), 2);

        let bytes = fs::read(&path).unwrap();
        assert_default_metadata(&bytes);
        let table = read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(73),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut expected = vec![Value::Int32(1)];
        expected.extend(if omit_companions {
            [
                Value::Int32(0),
                Value::Int64(0),
                Value::Currency(Currency::from_scaled(0)),
                Value::Boolean(false),
            ]
        } else {
            supplied
        });
        expected.push(Value::Null);
        assert_eq!(table.rows(), [expected]);
    }
}

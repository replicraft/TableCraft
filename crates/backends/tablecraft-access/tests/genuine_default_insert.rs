#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_access::editing::offline::{OfflineWriteSession, Phase};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{EditableTable, InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::genuine_default_insert::{
    self as sources, RECIPES, Recipe,
};

const SCOPE: CatalogScope = CatalogScope::new(201);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn verify(raw: &[u8], expected: &str) {
    let actual = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}

fn values(recipe: Recipe) -> Vec<InsertValue> {
    recipe
        .values()
        .into_iter()
        .map(|field| match field {
            Some(value) => InsertValue::Supplied(value),
            None => InsertValue::Omitted,
        })
        .collect()
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn typed_presence_and_sql_partial_fields_match_all_literal_images_under_the_same_lease() {
    let cases = sources::load_all(verify);
    let mut commits = 0;
    for case in cases {
        for recipe in RECIPES {
            for typed in [true, false] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let inserter = session
                        .insert_table("Defaults", &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(inserter.name(), "Defaults");
                    assert_eq!(
                        inserter
                            .columns()
                            .iter()
                            .map(|column| column.name())
                            .collect::<Vec<_>>(),
                        ["Id", "Score", "Caption", "Flag", "Money", "Optional"]
                    );
                    assert!(!inserter.requires_field_list());
                    inserter
                        .plan_insert_with_omissions(values(recipe), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(recipe.sql(), ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(receipt.matched_rows(), None);
                assert_eq!(receipt.changed_rows(), Some(1));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), case.expected(recipe));
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                let editor = session
                    .edit_table("Defaults", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    EditableTable::snapshot(&editor).rows(),
                    vec![sources::original_values(), recipe.final_values()]
                );
                let session = editor.into_session();
                assert_eq!(session.last_completed(), Some(receipt));
                session.release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                commits += 1;
            }
        }
    }
    assert_eq!(commits, 12);
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn missing_manual_id_and_supplied_null_boolean_refuse_without_defaulting_or_releasing_owner() {
    let mut refusals = 0;
    for case in sources::load_all(verify) {
        for missing_id in [false, true] {
            for typed in [true, false] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("target.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let failure = if typed {
                    let inserter = session
                        .insert_table("Defaults", &mut budget(), &|| false)
                        .unwrap();
                    let mut original = values(Recipe::OmittedConstants);
                    if missing_id {
                        original[0] = InsertValue::Omitted;
                    } else {
                        original[3] = InsertValue::Supplied(Value::Null);
                    }
                    inserter
                        .plan_insert_with_omissions(original, &mut budget(), &|| false)
                        .err()
                        .unwrap()
                } else {
                    let sql = if missing_id {
                        "INSERT INTO Defaults ([Score]) VALUES (7)"
                    } else {
                        "INSERT INTO Defaults ([Id], [Flag]) VALUES (24, NULL)"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .err()
                        .unwrap()
                };
                assert_eq!(failure.phase(), Phase::Planning);
                assert_eq!(failure.current_outcome(), None);
                assert_eq!(failure.last_completed(), None);
                assert_eq!(failure.pair_state(), None);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
                failure.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
                refusals += 1;
            }
        }
    }
    assert_eq!(refusals, 8);
}

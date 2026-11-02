//! Original presence remains journal intent; all AFTERs were authored before producers.
#[path = "genuine_default_insert/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, InsertFieldValue, NativeImage, NativeOmittedInsertIntent,
    NativeRecoveryQualificationError, OmittedRowInsert, RecoverableEditError, WriteBlocker,
    qualify_native_omitted_insert_recovery, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_default_insert::{
    self as sources, Case, RECIPES, Recipe,
};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(recipe: Recipe) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(
        recipe
            .values()
            .into_iter()
            .map(|field| match field {
                Some(value) => InsertFieldValue::Supplied(value),
                None => InsertFieldValue::Omitted,
            })
            .collect(),
    )]
}

// Independent wire fields are literal and never derived from codec constants.
fn wire(before: &[u8], recipe: Option<Recipe>) -> Vec<u8> {
    let row_bytes = match recipe {
        None => 0,
        Some(Recipe::OmittedConstants) => 60,
        Some(Recipe::ExplicitNull) => 61,
        Some(Recipe::AllOverrides) => 85,
    };
    let mut raw = Vec::from(*b"TCNA0002");
    raw.extend_from_slice(&(76_u32 + row_bytes).to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    for field in [8_u32, u32::from(recipe.is_some()), 0] {
        raw.extend_from_slice(&field.to_le_bytes());
    }
    raw.extend_from_slice(b"Defaults");
    if let Some(recipe) = recipe {
        raw.extend_from_slice(&row_bytes.to_le_bytes());
        raw.extend_from_slice(&6_u32.to_le_bytes());
        for field in recipe.values() {
            let (tag, payload) = match field {
                None => (8, Vec::new()),
                Some(Value::Null) => (0, Vec::new()),
                Some(Value::Boolean(value)) => (1, vec![u8::from(value)]),
                Some(Value::Int32(value)) => (2, value.to_le_bytes().to_vec()),
                Some(Value::Currency(value)) => (4, value.scaled().to_le_bytes().to_vec()),
                Some(Value::Text(value)) => (6, value.into_bytes()),
                _ => panic!("source recipe introduced a carrier without an independent wire law"),
            };
            raw.extend_from_slice(&[tag, 0, 0, 0]);
            raw.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            raw.extend_from_slice(&payload);
        }
    }
    assert_eq!(raw.len(), (76 + row_bytes) as usize);
    raw
}

fn intent(case: &Case, recipe: Recipe) -> NativeOmittedInsertIntent {
    NativeOmittedInsertIntent::decode(&wire(&case.before, Some(recipe)), &mut budget(), &|| false)
        .unwrap()
}

fn assert_rows(raw: &[u8], recipe: Recipe) {
    let expected = vec![sources::original_values(), recipe.final_values()];
    let sequential = read_table(
        &mut Cursor::new(raw),
        scope(),
        "Defaults",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequential.rows(), expected);
    for name in ["PrimaryID", "PrimaryAlias"] {
        let indexed = read_table_index(
            &mut Cursor::new(raw),
            scope(),
            "Defaults",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexed.rows(), expected);
    }
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn original_presence_commits_six_literal_images_and_empty_still_qualifies() {
    let mut commits = 0;
    for case in cases() {
        for recipe in RECIPES {
            let expected = case.expected(recipe);
            let plan = image(&case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Defaults",
                    requests(recipe),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.inserted_rows(), 1);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case.before, Some(recipe))
            );
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                1
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, expected, Marker::Committed)
            );
            assert_rows(target.bytes.get_ref(), recipe);
            commits += 1;
        }
        let empty = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Defaults",
                vec![],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);
        assert_eq!(
            empty.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before, None)
        );
    }
    assert_eq!(commits, 6);
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn explicit_null_boolean_missing_required_and_generated_requests_refuse_before_storage() {
    for case in cases() {
        let mut null_boolean = Recipe::OmittedConstants.values();
        null_boolean[3] = Some(Value::Null);
        let rows = vec![OmittedRowInsert::new(
            null_boolean
                .into_iter()
                .map(|value| match value {
                    Some(value) => InsertFieldValue::Supplied(value),
                    None => InsertFieldValue::Omitted,
                })
                .collect(),
        )];
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                "Defaults",
                rows,
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
        let missing = vec![OmittedRowInsert::new(
            (0..6).map(|_| InsertFieldValue::Omitted).collect(),
        )];
        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Defaults",
                    missing,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        // Actual metadata contains AutoNumber at native position1. Missing a
        // request is not permission to guess Increment/Random or consume LAST.
        for (table, width) in [("Numbered", 3), ("NonPrimary", 2)] {
            let rows = vec![OmittedRowInsert::new(
                (0..width).map(|_| InsertFieldValue::Omitted).collect(),
            )];
            assert!(
                image(&case.before)
                    .plan_recoverable_ordinary_omitted_row_inserts(
                        table,
                        rows,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
    }
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn original_presence_replans_same_checked_journal_before_recovery_and_confirmation() {
    for case in cases() {
        for recipe in RECIPES {
            let expected = case.expected(recipe);
            // A complete AFTER target can roll back pending or confirm committed.
            // Mixed targets below have a different committed recovery contract.
            for marker in [Marker::Pending, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let settled = if committed { expected } else { &case.before };
                let mut target = Storage::new(expected.to_vec());
                let mut journal = Storage::new(journals::journal(&case.before, expected, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_omitted_insert_recovery(
                        checked,
                        intent(&case, recipe),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .recover(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        outcome
                    );
                    assert_eq!(target.bytes.get_ref(), settled);
                }
                let writes = target.writes;
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_omitted_insert_recovery(
                    checked,
                    intent(&case, recipe),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .confirm_completed(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    outcome
                );
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
            // Copy exactly the changed DATA page, retaining old TD and INDEX.
            let mut mixed = case.before.clone();
            let start = case.data as usize * 4096;
            mixed[start..start + 4096].copy_from_slice(&expected[start..start + 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, expected);
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut target = Storage::new(mixed.clone());
                let mut journal = Storage::new(journals::journal(&case.before, expected, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_omitted_insert_recovery(
                        checked,
                        intent(&case, recipe),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let recovered = qualified.recover(&mut target, &mut budget(), &|| false);
                    if committed {
                        assert!(matches!(recovered, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.bytes.get_ref(), &mixed);
                        assert_eq!(target.writes, 0);
                    } else {
                        assert_eq!(recovered.unwrap(), RecoveryOutcome::RolledBack);
                        assert_eq!(target.bytes.get_ref(), &case.before);
                    }
                }
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

#[test]
#[ignore = "requires eight pinned default-presence images and independent SDK32 qualification"]
fn forged_protected_original_payload_index_history_and_final_bytes_refuse() {
    for case in cases() {
        for recipe in RECIPES {
            let expected = case.expected(recipe);
            for offset in [
                case.definition as usize * 4096 + 20,
                case.definition as usize * 4096 + 67,
                case.data as usize * 4096 + 4053,
                case.index as usize * 4096 + 481,
                expected.len() - 1,
            ] {
                let mut forged = expected.to_vec();
                forged[offset] ^= 1;
                let mut journal =
                    Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_omitted_insert_recovery(
                        checked,
                        intent(&case, recipe),
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

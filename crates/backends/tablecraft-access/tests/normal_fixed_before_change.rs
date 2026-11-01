//! Proposed integration child; SOURCE owns every complete expected image.
#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, io::Cursor};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_access::editing::offline::{OfflineWriteSession, Phase};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_io::{ExclusiveFile, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_fixed_before_change::{self as model, Recipe},
    canonical_mixed_before_change as legacy,
};

const SCOPE: CatalogScope = CatalogScope::new(118);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn locked(path: &std::path::Path) {
    assert!(ExclusiveFile::open(path).is_err());
}

#[test]
fn stored_scalar_editor_retains_old_single_int32_records_including_empty_requests() {
    for header in [5, 6] {
        let case = legacy::source(header, legacy::Recipe::CounterPrimary);
        for empty in [false, true] {
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let editor =
                NativeJournalEditor::new(image, "Items", &mut budget(), &|| false).unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            let updates = if empty {
                Vec::new()
            } else {
                vec![CellUpdate::new(
                    editor.snapshot().row_handle(0).unwrap(),
                    Value::Int32(17),
                )]
            };
            let plan = editor
                .plan_updates("Score", &updates, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16BeforeChangeInt32
            );
            assert!(plan.intent().fixed().is_none());
            let original = plan.intent().int32().unwrap();
            assert_eq!(original.column(), "Score");
            assert_eq!(original.updates().len(), usize::from(!empty));
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0002");
            let expected = if empty {
                &case.before
            } else {
                case.expected.as_ref().unwrap()
            };
            assert_eq!(plan.after_bytes(), expected);
        }
    }
}

#[test]
fn fixed_batch_sessions_publish_fifty_six_literal_images_in_both_group_orders() {
    let mut saves = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            for reverse in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &case.before).unwrap();
                let mut work = budget();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut work, &|| false).unwrap();
                let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), case.rows);
                let mut order: Vec<_> = (0..case.groups.len()).collect();
                if reverse {
                    order.reverse();
                }
                let replacements: Vec<Vec<CellUpdate>> = order
                    .iter()
                    .map(|&index| {
                        case.groups[index]
                            .updates
                            .iter()
                            .map(|update| {
                                let handle = editor.snapshot().row_handle(update.ordinal).unwrap();
                                assert_eq!(
                                    editor.snapshot().resolve(handle).unwrap()
                                        [case.groups[index].column],
                                    update.expected.value()
                                );
                                CellUpdate::new(handle, update.replacement.value())
                            })
                            .collect()
                    })
                    .collect();
                let groups: Vec<_> = order
                    .iter()
                    .zip(&replacements)
                    .map(|(&index, updates)| {
                        let field = model::FIELDS
                            .iter()
                            .find(|field| field.0 == case.groups[index].column)
                            .unwrap();
                        ColumnUpdates {
                            column: field.1,
                            updates,
                        }
                    })
                    .collect();
                let staged = editor.plan_columns(&groups, &mut work, &|| false).unwrap();
                assert_eq!(staged.matched_rows(), case.event_rows.len());
                assert_eq!(staged.changed_rows(), case.changed_rows);
                assert_eq!(fs::read(&path).unwrap(), case.before);
                assert!(!bundle.exists());
                locked(&path);

                let (session, receipt) = staged.apply(&bundle, &mut work, &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(fs::read(&path).unwrap(), *expected);
                assert!(!bundle.exists());
                locked(&path);
                let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
                assert_eq!(editor.snapshot().rows(), case.final_rows.as_ref().unwrap());
                editor.into_session().release().unwrap();
                assert_eq!(fs::read(&path).unwrap(), *expected);
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 56);
}

#[test]
fn fixed_batch_rejects_representable_refusals_before_journal_creation() {
    // StaleLate's explicit expected-value mismatch belongs to the native intent
    // API: typed snapshot handles correctly obtain the real expected value.
    for header in [5, 6] {
        for recipe in [
            Recipe::EqualDate,
            Recipe::EqualInactiveDate,
            Recipe::WrongCarrier,
            Recipe::DuplicateColumn,
        ] {
            let case = model::source(header, recipe);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            fs::write(&path, &case.before).unwrap();
            let mut work = budget();
            let session = OfflineWriteSession::open(&path, SCOPE, &mut work, &|| false).unwrap();
            let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
            let replacements: Vec<Vec<CellUpdate>> = case
                .groups
                .iter()
                .map(|group| {
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            CellUpdate::new(
                                editor.snapshot().row_handle(update.ordinal).unwrap(),
                                update.replacement.value(),
                            )
                        })
                        .collect()
                })
                .collect();
            let groups: Vec<_> = case
                .groups
                .iter()
                .zip(&replacements)
                .map(|(group, updates)| {
                    let field = model::FIELDS
                        .iter()
                        .find(|field| field.0 == group.column)
                        .unwrap();
                    ColumnUpdates {
                        column: field.1,
                        updates,
                    }
                })
                .collect();
            let failure = editor
                .plan_columns(&groups, &mut work, &|| false)
                .err()
                .expect("the complete intent must refuse");
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(fs::read(&path).unwrap(), case.before);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&path);
            failure.into_session().release().unwrap();
        }
    }
}

#[test]
fn fixed_batch_preserves_stale_handle_and_cancellation_boundaries() {
    let case = model::source(6, Recipe::CurrencyMaximum);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("target.accdb");
    fs::write(&path, &case.before).unwrap();
    let mut work = budget();
    let session = OfflineWriteSession::open(&path, SCOPE, &mut work, &|| false).unwrap();
    let editor = session.edit_table("Items", &mut work, &|| false).unwrap();
    let stale = editor.snapshot().row_handle(0).unwrap();
    let editor = editor
        .into_session()
        .edit_table("Items", &mut work, &|| false)
        .unwrap();
    let updates = [CellUpdate::new(
        stale,
        Value::Currency(tablecraft_core::values::Currency::from_scaled(i64::MAX)),
    )];
    let failure = editor
        .plan_columns(
            &[ColumnUpdates {
                column: "Money",
                updates: &updates,
            }],
            &mut work,
            &|| false,
        )
        .err()
        .expect("a handle from an abandoned snapshot is stale");
    assert_eq!(failure.phase(), Phase::Planning);
    assert_eq!(failure.pair_state(), None);
    let editor = failure
        .into_session()
        .edit_table("Items", &mut work, &|| false)
        .unwrap();
    let handle = editor.snapshot().row_handle(0).unwrap();
    let updates = [CellUpdate::new(
        handle,
        Value::Currency(tablecraft_core::values::Currency::from_scaled(i64::MAX)),
    )];
    let failure = editor
        .plan_columns(
            &[ColumnUpdates {
                column: "Money",
                updates: &updates,
            }],
            &mut work,
            &|| true,
        )
        .err()
        .expect("entry cancellation refuses before a journal");
    assert_eq!(failure.phase(), Phase::Planning);
    assert_eq!(failure.pair_state(), None);
    assert_eq!(fs::read(&path).unwrap(), case.before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    locked(&path);
    failure.into_session().release().unwrap();
}

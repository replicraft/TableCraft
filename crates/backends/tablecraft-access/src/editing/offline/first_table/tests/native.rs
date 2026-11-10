//! Native first-table schema uses the real session owner and checked V3 routes.
use super::*;
use crate::editing::offline::test_support::native_creation::wide_definition;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

#[test]
fn native_wide_first_table_applies_exact_image_and_reopens_all_fields() {
    let source = raw::source();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("wide.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.before).unwrap();
    #[cfg(unix)]
    let identity = fs::metadata(&path).unwrap();
    let staged = open(&path)
        .plan_first_table_if_original(
            wide_definition("Table1", &mut budget()),
            &source.before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(staged.plan.is_native());
    assert_eq!(staged.column_count(), 255);
    assert_eq!(staged.plan.after_bytes(), source.after);
    assert_staged_read_state(&path, &source.before);
    let intent = staged.plan.encode_intent(&mut budget(), &|| false).unwrap();
    assert!(
        matches!(MutationIntent::decode(&intent, &mut budget(), &|| false), Ok(MutationIntent::FirstTable(intent)) if intent.is_native())
    );
    let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(
        receipt.schema_change(),
        Some(SchemaChange::FirstTableCreated { column_count: 255 })
    );
    assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    assert_completion(&receipt);
    session.release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), source.after);
    #[cfg(unix)]
    {
        let after = fs::metadata(&path).unwrap();
        assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
    }
    let table = read_table(
        &mut Cursor::new(fs::read(&path).unwrap()),
        CatalogScope::new(7191),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().columns().len(), 255);
    assert!(table.rows().is_empty());
    assert_terminal_entries(directory.path(), 1);
}

#[test]
fn native_wide_first_table_cold_recovery_and_retired_confirmation_select_v3() {
    let source = raw::source();
    for committed in [false, true] {
        for retire in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("wide.accdb");
            let bundle = directory.path().join("journal");
            retained_pair_with_definition(
                &path,
                &bundle,
                committed,
                retire,
                wide_definition("Table1", &mut budget()),
            )
            .release()
            .unwrap();
            let (session, receipt) = if retire {
                let retired = fs::read_dir(directory.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.starts_with(".tablecraft-retired-"))
                    })
                    .unwrap();
                open(&path)
                    .confirm_retired(&retired, &mut budget(), &|| false)
                    .unwrap()
            } else {
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(
                receipt.kind(),
                if retire {
                    CompletionKind::Confirmed
                } else {
                    CompletionKind::Recovered
                }
            );
            assert_eq!(
                receipt.schema_change(),
                committed.then_some(SchemaChange::FirstTableCreated { column_count: 255 })
            );
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(
                receipt.outcome(),
                if retire {
                    None
                } else {
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                }
            );
            assert_eq!(
                receipt.verified_image(),
                retire.then_some(if committed {
                    VerifiedImage::Replacement
                } else {
                    VerifiedImage::Original
                })
            );
            assert_completion(&receipt);
            assert_eq!(session.state(), SessionState::Ready);
            session.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed {
                    &source.after
                } else {
                    &source.before
                }
                .as_slice()
            );
            assert_terminal_entries(directory.path(), 1);
        }
    }
}

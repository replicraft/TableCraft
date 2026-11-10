//! Native append preserves actual source cohorts through file-backed V3 sessions.
use super::*;
use crate::editing::offline::{VerifiedImage, test_support::native_creation::wide_definition};
use tablecraft_ace::AddTableSourceProfile;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::fields, canonical_wide_table_creation as raw,
};

fn sources() -> [(Vec<u8>, AddTableSourceProfile); 2] {
    [
        (raw::source().after, AddTableSourceProfile::Mixed),
        (
            fields::source(fields::Case::FirstNumber).bytes,
            AddTableSourceProfile::Retained,
        ),
    ]
}

#[test]
fn native_wide_append_applies_exact_mixed_and_retained_images_without_replacing_file() {
    for (before, profile) in sources() {
        let expected = raw::appended::source(&before, "More").after;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wide.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        #[cfg(unix)]
        let identity = fs::metadata(&path).unwrap();
        let staged = open(&path)
            .plan_add_table_if_original(
                wide_definition("More", &mut budget()),
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(staged.plan.is_native());
        assert_eq!(staged.plan.source_profile(), profile);
        assert_eq!(staged.plan.after_bytes(), expected);
        assert_staged_read_state(&path, &before);
        let intent = staged.plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert!(
            matches!(MutationIntent::decode(&intent, &mut budget(), &|| false), Ok(MutationIntent::AddTable(intent)) if intent.is_native() && intent.source_profile() == profile)
        );
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            receipt.schema_change(),
            Some(SchemaChange::TableAdded { column_count: 255 })
        );
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        #[cfg(unix)]
        {
            let after = fs::metadata(&path).unwrap();
            assert_eq!((after.dev(), after.ino()), (identity.dev(), identity.ino()));
        }
        let table = read_table(
            &mut Cursor::new(fs::read(&path).unwrap()),
            CatalogScope::new(7201),
            "More",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(table.schema().columns().len(), 255);
        assert!(table.rows().is_empty());
        assert_terminal_entries(directory.path(), 1);
    }
}

fn retained_native_pair(
    path: &Path,
    bundle: &Path,
    before: &[u8],
    committed: bool,
    retire: bool,
) -> OfflineWriteSession {
    fs::write(path, before).unwrap();
    let mut session = open(path);
    let slot = session.slot_mut().unwrap();
    let image = NativeImage::capture_retained(
        slot.target.as_mut().unwrap(),
        slot.scope,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let plan = image
        .plan_add_table_native(
            wide_definition("More", &mut budget()),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let intent = plan.encode_intent(&mut budget(), &|| false).unwrap();
    assert_eq!(&intent[..8], b"TCAT0007");
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
            .prepare_native(&mut target, &mut journal, &mut budget(), &|| false)
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
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    session
}

#[test]
fn native_wide_append_cold_recovery_and_retired_confirmation_keep_original_cohort() {
    for (before, _) in sources() {
        let expected = raw::appended::source(&before, "More").after;
        for committed in [false, true] {
            for retire in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("wide.accdb");
                let bundle = directory.path().join("journal");
                retained_native_pair(&path, &bundle, &before, committed, retire)
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
                    committed.then_some(SchemaChange::TableAdded { column_count: 255 })
                );
                assert_eq!(
                    receipt.verified_image(),
                    retire.then_some(if committed {
                        VerifiedImage::Replacement
                    } else {
                        VerifiedImage::Original
                    })
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
                assert_completion(&receipt);
                assert_eq!(session.state(), SessionState::Ready);
                session.release().unwrap();
                assert_eq!(
                    fs::read(&path).unwrap(),
                    if committed { &expected } else { &before }.as_slice()
                );
                assert_terminal_entries(directory.path(), 1);
            }
        }
    }
}

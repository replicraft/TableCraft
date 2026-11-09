use super::*;

#[test]
fn multi_fragment_retained_cold_and_settled_recovery_keep_complete_query_facts() {
    let before = raw::two_projections();
    for committed in [false, true] {
        for (retired, cold) in [(false, false), (false, true), (true, true)] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("recover.accdb");
            let bundle = directory.path().join("journal");
            let owner = retained_pair(&path, &bundle, &before, committed, retired);
            let (session, receipt) = if retired {
                owner.release().unwrap();
                open(&path)
                    .confirm_retired(&retired_bundle(directory.path()), &mut budget(), &|| false)
                    .unwrap()
            } else if cold {
                owner.release().unwrap();
                open(&path)
                    .recover_update(&bundle, &mut budget(), &|| false)
                    .unwrap()
            } else {
                owner.recover_pending(&mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.schema_change(), None);
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (None, None)
            );
            assert_eq!(
                receipt.query_change(),
                committed.then_some(QueryChange::ExpressionSaved)
            );
            if retired {
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
            } else {
                assert_eq!(receipt.kind(), CompletionKind::Recovered);
                assert_eq!(
                    receipt.outcome(),
                    Some(if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    })
                );
            }
            assert_completion(&receipt);
            session.release().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                if committed {
                    after(&before)
                } else {
                    before.clone()
                }
            );
        }
    }
}

#[test]
fn syntactically_valid_partial_fragment_after_never_gains_recovery_authority() {
    let before = raw::two_projections();
    let partial = raw::replace_at(&before, 4, "7000");
    let intent = review(&before, true)
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("partial.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let mut owner = open(&path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &bundle,
        intent,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.storage().unwrap();
        drop(
            tablecraft_io::journal::prepare_expected(
                &mut target,
                &mut journal,
                &before,
                &partial,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner.release().unwrap();
    let failure = open(&path)
        .recover_update(&bundle, &mut budget(), &|| false)
        .err()
        .expect("both edits must reconstruct");
    assert_eq!(failure.phase(), Phase::Recovery);
    assert!(matches!(
        failure.cause(),
        Cause::QueryExpressionEditsIntent(_)
    ));
    assert_eq!(failure.current_outcome(), None);
    assert!(failure.pair_state().is_some());
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(bundle.exists());
}

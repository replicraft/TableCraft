use super::*;

#[test]
fn multi_fragment_apply_and_noop_reuse_query_only_staging_and_receipts() {
    let before = raw::two_projections();
    for changed in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("query.accdb");
        let bundle = directory.path().join("journal");
        fs::write(&path, &before).unwrap();
        let staged = open(&path)
            .stage_reviewed_query_expression_edits_with_budgets(
                review(&before, changed),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(staged.query(), "Saved");
        assert_eq!(staged.native_id(), -2147483525);
        assert_eq!(staged.changed(), changed);
        assert_staged_read_state(&path, &before);
        assert!(!bundle.exists());
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (None, None)
        );
        assert_eq!(receipt.schema_change(), None);
        assert_eq!(
            receipt.query_change(),
            changed.then_some(QueryChange::ExpressionSaved)
        );
        assert_completion(&receipt);
        session.release().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            if changed {
                after(&before)
            } else {
                before.clone()
            }
        );
    }
}

#[test]
fn fresh_source_and_separate_stage_budgets_refuse_without_creating_journal() {
    let before = raw::two_projections();
    for case in 0..4 {
        let reviewed = review(&before, true);
        let mut current = before.clone();
        if case == 0 {
            *current.last_mut().unwrap() ^= 1;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("refused.accdb");
        fs::write(&path, &current).unwrap();
        let mut native = if case == 1 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if case == 2 {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let failure = open(&path)
            .stage_reviewed_query_expression_edits_with_budgets(
                reviewed,
                &mut native,
                &mut proof,
                &|| case == 3,
            )
            .err()
            .expect("fresh source/cancellation/quota must refuse");
        assert_eq!(failure.phase(), Phase::Planning);
        assert_eq!(failure.pair_state(), None);
        assert_eq!(failure.current_outcome(), None);
        assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        failure.into_session().release().unwrap();
        assert_eq!(fs::read(&path).unwrap(), current);
    }
}

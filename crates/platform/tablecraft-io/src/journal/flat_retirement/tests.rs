use super::*;

#[derive(Default)]
struct Ops {
    calls: Vec<&'static str>,
    verify_fail: bool,
    rename_error: Option<ErrorKind>,
    flush_failures: usize,
}

impl NamespaceOps for Ops {
    fn verify(&mut self) -> Result<(), PairError> {
        self.calls.push("verify");
        if self.verify_fail {
            Err(PairError::IdentityMismatch)
        } else {
            Ok(())
        }
    }

    fn rename(&mut self) -> Result<(), PairError> {
        self.calls.push("rename");
        match self.rename_error {
            Some(kind) => Err(IoError::Io(kind).into()),
            None => Ok(()),
        }
    }

    fn flush(&mut self) -> Result<(), PairError> {
        self.calls.push("flush");
        if self.flush_failures > 0 {
            self.flush_failures -= 1;
            Err(IoError::Io(ErrorKind::Other).into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_core_retirement_preserves_each_outcome_and_rejects_replay() {
    for outcome in [RecoveryOutcome::Committed, RecoveryOutcome::RolledBack] {
        let mut permit = TerminalPermit::reserved(false);
        let mut state = PairState::RecoveryRequired;
        let mut known = None;
        let mut ops = Ops::default();
        permit
            .retire(&mut state, &mut known, outcome, &mut ops)
            .unwrap();
        assert_eq!(ops.calls, ["verify", "rename", "flush"]);
        assert_eq!(state, PairState::Retired);
        assert_eq!(known, Some(outcome));
        let failure = permit
            .retire(&mut state, &mut known, outcome, &mut ops)
            .unwrap_err();
        assert_eq!(failure.phase, FinalizationPhase::State);
        assert_eq!(failure.outcome, Some(outcome));
        assert_eq!(ops.calls, ["verify", "rename", "flush"]);
    }
}

#[test]
fn verify_failure_collision_and_unknown_rename_never_replay_or_flush() {
    for (verify_fail, rename_error, expected, calls) in [
        (true, None, PairState::RetirementBlocked, vec!["verify"]),
        (
            false,
            Some(ErrorKind::AlreadyExists),
            PairState::RetirementBlocked,
            vec!["verify", "rename"],
        ),
        (
            false,
            Some(ErrorKind::Other),
            PairState::RetirementUncertain,
            vec!["verify", "rename"],
        ),
    ] {
        let mut permit = TerminalPermit::reserved(false);
        let mut state = PairState::RecoveryRequired;
        let mut known = None;
        let mut ops = Ops {
            verify_fail,
            rename_error,
            ..Default::default()
        };
        let failure = permit
            .retire(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
            .unwrap_err();
        assert_eq!(failure.phase, FinalizationPhase::Retirement);
        assert_eq!(failure.outcome, Some(RecoveryOutcome::Committed));
        assert_eq!(state, expected);
        assert_eq!(ops.calls, calls);
        assert!(
            permit
                .retire(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert!(
            permit
                .confirm(&mut state, known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert_eq!(ops.calls, calls);
    }
}

#[test]
fn acknowledged_rename_allows_exactly_one_explicit_barrier_retry_without_rename() {
    for failures in [1, 2] {
        let mut permit = TerminalPermit::reserved(false);
        let mut state = PairState::RecoveryRequired;
        let mut known = None;
        let mut ops = Ops {
            flush_failures: failures,
            ..Default::default()
        };
        let failure = permit
            .retire(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
            .unwrap_err();
        assert_eq!(failure.phase, FinalizationPhase::RetirementBarrier);
        assert_eq!(state, PairState::RetirementUnconfirmed);
        assert!(
            permit
                .confirm(&mut state, known, RecoveryOutcome::RolledBack, &mut ops)
                .is_err()
        );
        assert_eq!(ops.calls, ["verify", "rename", "flush"]);
        let result = permit.confirm(&mut state, known, RecoveryOutcome::Committed, &mut ops);
        assert_eq!(result.is_ok(), failures == 1);
        assert_eq!(
            state,
            if failures == 1 {
                PairState::Retired
            } else {
                PairState::RetirementUnconfirmed
            }
        );
        assert!(
            permit
                .confirm(&mut state, known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert!(
            permit
                .retire(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert_eq!(known, Some(RecoveryOutcome::Committed));
        assert_eq!(ops.calls, ["verify", "rename", "flush", "flush"]);
    }
}

#[test]
fn retired_confirmation_verifies_and_flushes_without_any_rename_or_outcome_replay() {
    for failures in [0, 1, 2] {
        let mut permit = TerminalPermit::reserved(true);
        let mut state = PairState::RetiredVerificationRequired;
        let mut known = None;
        let mut ops = Ops {
            flush_failures: failures,
            ..Default::default()
        };
        let result = permit.acknowledge(
            &mut state,
            &mut known,
            RecoveryOutcome::RolledBack,
            &mut ops,
        );
        assert_eq!(result.is_ok(), failures == 0);
        assert_eq!(known, Some(RecoveryOutcome::RolledBack));
        assert_eq!(ops.calls, ["verify", "flush"]);
        assert!(
            permit
                .acknowledge(
                    &mut state,
                    &mut known,
                    RecoveryOutcome::RolledBack,
                    &mut ops
                )
                .is_err()
        );
        assert!(
            permit
                .retire(
                    &mut state,
                    &mut known,
                    RecoveryOutcome::RolledBack,
                    &mut ops
                )
                .is_err()
        );
        if failures > 0 {
            assert_eq!(
                permit
                    .confirm(&mut state, known, RecoveryOutcome::RolledBack, &mut ops)
                    .is_ok(),
                failures == 1
            );
            assert!(
                permit
                    .confirm(&mut state, known, RecoveryOutcome::RolledBack, &mut ops)
                    .is_err()
            );
            assert_eq!(ops.calls, ["verify", "flush", "flush"]);
        }
        assert!(!ops.calls.contains(&"rename"));
    }
}

#[test]
fn invalid_phases_and_mismatched_known_outcome_perform_no_callback() {
    for phase in [
        PairState::Ready,
        PairState::Released,
        PairState::ReleaseUncertain,
        PairState::RetiredClean,
        PairState::RetirementBlocked,
    ] {
        let mut permit = TerminalPermit::reserved(false);
        let mut state = phase;
        let mut known = None;
        let mut ops = Ops::default();
        assert!(
            permit
                .retire(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert!(
            permit
                .acknowledge(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert!(
            permit
                .confirm(&mut state, known, RecoveryOutcome::Committed, &mut ops)
                .is_err()
        );
        assert_eq!(state, phase);
        assert_eq!(known, None);
        assert!(ops.calls.is_empty());
    }
    let mut permit = TerminalPermit::reserved(false);
    let mut state = PairState::OutcomeRecorded;
    let mut known = Some(RecoveryOutcome::Committed);
    let mut ops = Ops::default();
    let failure = permit
        .retire(
            &mut state,
            &mut known,
            RecoveryOutcome::RolledBack,
            &mut ops,
        )
        .unwrap_err();
    assert_eq!(failure.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(state, PairState::OutcomeRecorded);
    assert!(ops.calls.is_empty());
}

#[test]
fn failed_retired_identity_verification_keeps_outcome_without_barrier_or_retry() {
    let mut permit = TerminalPermit::reserved(true);
    let mut state = PairState::RetiredVerificationRequired;
    let mut known = None;
    let mut ops = Ops {
        verify_fail: true,
        ..Default::default()
    };
    let failure = permit
        .acknowledge(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
        .unwrap_err();
    assert_eq!(failure.phase, FinalizationPhase::Retirement);
    assert_eq!(failure.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(state, PairState::RetirementBlocked);
    assert!(
        permit
            .confirm(&mut state, known, RecoveryOutcome::Committed, &mut ops)
            .is_err()
    );
    assert!(
        permit
            .acknowledge(&mut state, &mut known, RecoveryOutcome::Committed, &mut ops)
            .is_err()
    );
    assert_eq!(ops.calls, ["verify"]);
}

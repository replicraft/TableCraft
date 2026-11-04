//! Allocation-free outcome facts derived from the retained actual resource owner.
use super::RetainedOwner;
use crate::editing::save::{
    SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome, SaveOutcome, SavePhase,
    SaveReceipt, SaveSessionState,
};
use std::error::Error as StdError;
use tablecraft_access::editing::offline::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt,
    SessionState, VerifiedImage,
};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::LimitError;
use tablecraft_io::{
    IoError,
    journal::{RecoveryOutcome, filesystem::PairState},
};

pub(super) fn retain_local(
    session: OfflineWriteSession,
    kind: SaveFailureKind,
    retained: &mut Option<RetainedOwner>,
) -> SaveOutcome {
    let mut failure = no_session(kind);
    failure.phase = Some(SavePhase::Planning);
    failure.state = Some(SaveSessionState::Ready);
    failure.last_completed = session.last_completed().map(receipt);
    failure.can_release = true;
    *retained = Some(RetainedOwner::Session(session));
    SaveOutcome::Failed(failure)
}

pub(super) fn finish(
    result: Result<(OfflineWriteSession, Receipt), Failure>,
    retained: &mut Option<RetainedOwner>,
) -> SaveOutcome {
    match result {
        Ok((session, completed)) => match session.release() {
            Ok(()) => SaveOutcome::Saved {
                receipt: receipt(completed),
            },
            Err(failure) => retain(failure, retained),
        },
        Err(failure) => retain(failure, retained),
    }
}

pub(super) fn no_session(kind: SaveFailureKind) -> SaveFailure {
    SaveFailure {
        kind,
        phase: None,
        state: None,
        current_outcome: None,
        last_completed: None,
        can_recover: false,
        can_release: false,
    }
}

pub(super) fn known(value: KnownOutcome) -> SaveKnownOutcome {
    match value {
        KnownOutcome::Native(RecoveryOutcome::Committed) => SaveKnownOutcome::Committed,
        KnownOutcome::Native(RecoveryOutcome::RolledBack) => SaveKnownOutcome::RolledBack,
        KnownOutcome::Verified(VerifiedImage::Original) => SaveKnownOutcome::OriginalVerified,
        KnownOutcome::Verified(VerifiedImage::Replacement) => SaveKnownOutcome::ReplacementVerified,
    }
}

pub(super) fn receipt(value: Receipt) -> SaveReceipt {
    SaveReceipt {
        completion: match value.kind() {
            CompletionKind::Applied => SaveCompletion::Applied,
            CompletionKind::Recovered => SaveCompletion::Recovered,
            CompletionKind::Confirmed => SaveCompletion::Confirmed,
        },
        known: value
            .outcome()
            .map(|value| known(KnownOutcome::Native(value)))
            .unwrap_or_else(|| {
                known(KnownOutcome::Verified(
                    value
                        .verified_image()
                        .expect("confirmation has a verified image"),
                ))
            }),
        matched_rows: value.matched_rows(),
        changed_rows: value.changed_rows(),
    }
}

pub(super) fn retain(failure: Failure, retained: &mut Option<RetainedOwner>) -> SaveOutcome {
    let phase = match failure.phase() {
        Phase::Planning => SavePhase::Planning,
        Phase::Intent => SavePhase::Intent,
        Phase::Setup => SavePhase::Setup,
        Phase::Prepare => SavePhase::Prepare,
        Phase::Commit => SavePhase::Commit,
        Phase::Recovery => SavePhase::Recovery,
        Phase::Confirmation => SavePhase::Confirmation,
        Phase::Refresh => SavePhase::Refresh,
        Phase::Finalization => SavePhase::Finalization,
        Phase::Release => SavePhase::Release,
    };
    let state = match failure.state() {
        SessionState::Ready => SaveSessionState::Ready,
        SessionState::RefreshRequired => SaveSessionState::RefreshRequired,
        SessionState::RetainedJournal => SaveSessionState::RetainedJournal,
        SessionState::IncompleteSetup => SaveSessionState::IncompleteSetup,
        SessionState::ReleaseUncertain => SaveSessionState::ReleaseUncertain,
        SessionState::Released => SaveSessionState::Released,
    };
    let kind = resource_failure(failure.cause()).unwrap_or_else(|| match failure.cause() {
        Cause::SourceChanged => SaveFailureKind::SourceChanged,
        Cause::Native(InspectError::Cancelled) => SaveFailureKind::Cancelled,
        Cause::Native(
            InspectError::Limit(_) | InspectError::Allocation | InspectError::StructureLimit,
        ) => SaveFailureKind::Resource,
        Cause::InvalidState => SaveFailureKind::InvalidState,
        _ => match phase {
            SavePhase::Planning => SaveFailureKind::Planning,
            SavePhase::Intent => SaveFailureKind::Intent,
            SavePhase::Setup => SaveFailureKind::Setup,
            SavePhase::Prepare => SaveFailureKind::Prepare,
            SavePhase::Commit => SaveFailureKind::Commit,
            SavePhase::Recovery | SavePhase::Confirmation => SaveFailureKind::Recovery,
            SavePhase::Finalization => SaveFailureKind::Finalization,
            SavePhase::Release => SaveFailureKind::Release,
            SavePhase::Refresh => SaveFailureKind::Planning,
        },
    });
    let current_outcome = failure.current_outcome().map(known);
    let last_completed = failure.last_completed().map(receipt);
    let report = SaveFailure {
        kind,
        phase: Some(phase),
        state: Some(state),
        current_outcome,
        last_completed,
        can_recover: state == SaveSessionState::RetainedJournal
            && current_outcome.is_none()
            && matches!(
                failure.pair_state(),
                Some(PairState::Ready | PairState::RecoveryRequired)
            ),
        can_release: !matches!(
            state,
            SaveSessionState::ReleaseUncertain | SaveSessionState::Released
        ),
    };
    *retained = Some(RetainedOwner::Failure(failure));
    SaveOutcome::Failed(report)
}

// Error inspection is bounded and allocation-free even after storage effects.
// The full original Failure remains owned; this only derives display-safe facts.
fn resource_failure(error: &(dyn StdError + 'static)) -> Option<SaveFailureKind> {
    let mut current = Some(error);
    for _ in 0..32 {
        let error = current?;
        if error.downcast_ref::<LimitError>().is_some() {
            return Some(SaveFailureKind::Resource);
        }
        if let Some(error) = error.downcast_ref::<InspectError>() {
            match error {
                InspectError::Cancelled => return Some(SaveFailureKind::Cancelled),
                InspectError::Limit(_)
                | InspectError::Allocation
                | InspectError::StructureLimit => return Some(SaveFailureKind::Resource),
                _ => {}
            }
        }
        if let Some(error) = error.downcast_ref::<IoError>() {
            match error {
                IoError::Cancelled => return Some(SaveFailureKind::Cancelled),
                IoError::Limit(_) => return Some(SaveFailureKind::Resource),
                _ => {}
            }
        }
        current = error.source();
    }
    None
}

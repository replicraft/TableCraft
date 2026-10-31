use super::confirmation::VerifiedImage;
use std::{fmt, io};
use tablecraft_ace::{InspectError, NativeIntentError, NativeRecoveryQualificationError};
use tablecraft_io::{
    IoError,
    journal::{CommitError, JournalError, RecoveryOutcome},
};

use tablecraft_io::journal::filesystem::{FinalizationError, PairError, PairSetupFailure};

/// Distinguishes secondary lifecycle errors from the already known native fact.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FinalizationPhase {
    Retirement,
    Cleanup,
}

impl fmt::Display for FinalizationPhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Retirement => "journal retirement",
            Self::Cleanup => "retired journal cleanup",
        })
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    Credentials,
    RetiredOpen(Vec<PairSetupFailure>),
    RetiredPair(PairError),
    ConfirmationIntent(NativeIntentError),
    ConfirmationResources(InspectError),
    ConfirmationQualification(NativeRecoveryQualificationError),
    Confirmation(JournalError),
    VerifiedFinalization {
        image: VerifiedImage,
        error: FinalizationError,
    },
    VerifiedRelease {
        image: VerifiedImage,
        error: PairError,
    },
    VerifiedOutput {
        image: VerifiedImage,
        error: io::Error,
    },
    Parse(tablecraft_sql::ParseError),
    Shape(tablecraft_query::QueryError),
    Native(InspectError),
    Planning(tablecraft_access::editing::sql::RecoverableError),
    Intent(NativeIntentError),
    RecoveryIntent(NativeIntentError),
    RecoveryResources(InspectError),
    Io(IoError),
    Setup(PairSetupFailure),
    Pair(PairError),
    Prepare(JournalError),
    Commit(CommitError),
    Qualification(NativeRecoveryQualificationError),
    Recovery(JournalError),
    Finalization {
        outcome: RecoveryOutcome,
        phase: FinalizationPhase,
        error: FinalizationError,
    },
    Release {
        outcome: RecoveryOutcome,
        error: PairError,
    },
    Output {
        outcome: RecoveryOutcome,
        error: io::Error,
    },
}

impl Error {
    /// Ordinary preflight refusals are invalid-input exits. Journal-bearing
    /// failures and known outcomes use failure exits even if diagnostics fail.
    pub(crate) fn invalid_input(&self) -> bool {
        matches!(
            self,
            Self::Credentials
                | Self::Parse(_)
                | Self::Shape(_)
                | Self::Native(_)
                | Self::Planning(_)
                | Self::Intent(_)
                | Self::Io(_)
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RetiredOpen(retained) => {
                formatter.write_str(
                    "retired journal association refused; retain the target and remaining bundle for diagnosis",
                )?;
                if retained.is_empty() {
                    return formatter.write_str("; setup failure details are missing");
                }
                for error in retained {
                    write!(formatter, ": {error}")?;
                }
                Ok(())
            }
            Self::RetiredPair(error) => write!(
                formatter,
                "retired journal storage refused; no recovery fallback: {error}"
            ),
            Self::ConfirmationIntent(error) => write!(
                formatter,
                "retired intent refused; retain the target and remaining bundle: {error}"
            ),
            Self::ConfirmationResources(error) => write!(
                formatter,
                "confirmation preparation failed; retain the target and remaining bundle: {error}"
            ),
            Self::ConfirmationQualification(error) => write!(
                formatter,
                "retired journal qualification refused; no recovery replay: {error}"
            ),
            Self::Confirmation(error) => write!(
                formatter,
                "current-image confirmation failed; no recovery replay: {error}"
            ),
            Self::VerifiedFinalization { image, error } => write!(
                formatter,
                "verified target image {image}; namespace finalization failed ({:?}); do not rerun SQL: {}",
                error.phase, error.cause
            ),
            Self::VerifiedRelease { image, error } => write!(
                formatter,
                "verified target image {image}; exclusive release failed; do not rerun SQL: {error}"
            ),
            Self::VerifiedOutput { image, error } => write!(
                formatter,
                "verified target image {image}; completion output failed; do not rerun SQL: {error}"
            ),
            Self::Credentials => formatter.write_str(
                "journal editing accepts only unencoded Jet4 files; --password is unsupported",
            ),
            Self::Parse(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::Planning(error) => error.fmt(formatter),
            Self::Intent(error) => error.fmt(formatter),
            Self::RecoveryIntent(error) => write!(
                formatter,
                "recovery intent refused; retain the bundle and target for diagnosis: {error}"
            ),
            Self::RecoveryResources(error) => write!(
                formatter,
                "recovery preparation failed; retain the bundle and target: {error}"
            ),
            Self::Io(error) => error.fmt(formatter),
            Self::Setup(error) => write!(
                formatter,
                "journal setup incomplete; retain any bundle for diagnosis: {error}"
            ),
            Self::Pair(error) => write!(
                formatter,
                "journal association refused; target may still require recovery: {error}"
            ),
            Self::Prepare(error) => write!(
                formatter,
                "journal preparation failed; retain the bundle for diagnosis; target was not changed by preparation: {error}"
            ),
            Self::Commit(error) => write!(
                formatter,
                "recovery required; do not retry the UPDATE or delete the journal: {error}"
            ),
            Self::Qualification(error) => write!(
                formatter,
                "recovery qualification refused; retain the bundle and target for diagnosis: {error}"
            ),
            Self::Recovery(error) => write!(
                formatter,
                "recovery still required; retain the bundle and target: {error}"
            ),
            Self::Finalization {
                outcome,
                phase,
                error,
            } => write!(
                formatter,
                "target outcome {outcome:?}; {phase} failed; do not retry the UPDATE: {error}"
            ),
            Self::Release { outcome, error } => write!(
                formatter,
                "target outcome {outcome:?}; exclusive resource release failed; do not retry the UPDATE: {error}"
            ),
            Self::Output { outcome, error } => write!(
                formatter,
                "target outcome {outcome:?}; completion output failed; do not retry the UPDATE: {error}"
            ),
        }
    }
}

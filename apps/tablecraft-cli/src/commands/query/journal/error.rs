use super::confirmation::VerifiedImage;
use std::{fmt, io};
use tablecraft_access::editing::offline::{Failure, OfflineWriteSession, OpenError, Receipt};
use tablecraft_ace::InspectError;
use tablecraft_io::journal::RecoveryOutcome;

pub(crate) struct UnexpectedReceipt {
    pub session: OfflineWriteSession,
    pub receipt: Receipt,
}

impl fmt::Debug for UnexpectedReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UnexpectedReceipt")
            .field("state", &self.session.state())
            .field("receipt", &self.receipt)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    Credentials,
    ConfirmationResources(InspectError),
    VerifiedOutput {
        image: VerifiedImage,
        error: io::Error,
    },
    Parse(tablecraft_sql::ParseError),
    Shape(tablecraft_query::QueryError),
    Native(InspectError),
    RecoveryResources(InspectError),
    OfflineOpen(OpenError),
    Offline(Failure),
    UnexpectedReceipt(UnexpectedReceipt),
    Output {
        outcome: RecoveryOutcome,
        error: io::Error,
    },
}

impl Error {
    /// Ordinary preflight refusals are invalid-input exits. Journal-bearing
    /// failures and known outcomes use failure exits even if diagnostics fail.
    pub(crate) fn invalid_input(&self) -> bool {
        if let Self::Offline(failure) = self {
            return failure.invalid_input();
        }
        matches!(
            self,
            Self::Credentials
                | Self::Parse(_)
                | Self::Shape(_)
                | Self::Native(_)
                | Self::OfflineOpen(_)
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VerifiedOutput { image, error } => write!(
                formatter,
                "verified target image {image}; completion output failed; do not rerun SQL: {error}"
            ),
            Self::Credentials => formatter.write_str(
                "journal editing requires an unencoded admitted Jet4 or ACE BeforeChange profile; --password is unsupported",
            ),
            Self::Parse(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::RecoveryResources(error) => write!(
                formatter,
                "recovery preparation failed; retain the bundle and target: {error}"
            ),
            Self::ConfirmationResources(error) => write!(
                formatter,
                "confirmation preparation failed; retain the target and remaining bundle: {error}"
            ),
            Self::OfflineOpen(error) => error.fmt(formatter),
            Self::Offline(error) => error.fmt(formatter),
            Self::UnexpectedReceipt(retained) => write!(
                formatter,
                "unexpected offline completion {:?}; target retained in state {:?}; do not retry the UPDATE",
                retained.receipt,
                retained.session.state()
            ),
            Self::Output { outcome, error } => write!(
                formatter,
                "target outcome {outcome:?}; completion output failed; do not retry the UPDATE: {error}"
            ),
        }
    }
}

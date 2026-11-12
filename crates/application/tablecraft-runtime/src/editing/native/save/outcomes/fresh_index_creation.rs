//! Transparent fresh-original layers preserve cancellation and resource identity.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{EditError, FreshIndexCreateIntentError as Error};

pub(super) fn intent(error: &Error) -> Option<SaveFailureKind> {
    match error {
        Error::Inspect(error) => inspection_failure(error),
        Error::Binding(error) => intent_failure(error),
        Error::Edit(EditError::Inspect(error)) => inspection_failure(error),
        Error::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

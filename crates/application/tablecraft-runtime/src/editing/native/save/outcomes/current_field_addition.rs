//! Current source and original request retain typed cancellation/resource facts.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{CurrentFieldAdditionError, CurrentFieldAdditionIntentError, EditError};

pub(super) fn intent(error: &CurrentFieldAdditionIntentError) -> Option<SaveFailureKind> {
    match error {
        CurrentFieldAdditionIntentError::Inspect(error) => inspection_failure(error),
        CurrentFieldAdditionIntentError::Binding(error) => intent_failure(error),
        CurrentFieldAdditionIntentError::Proof(error) => qualification_failure(error),
        CurrentFieldAdditionIntentError::Field(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &CurrentFieldAdditionError) -> Option<SaveFailureKind> {
    match error {
        CurrentFieldAdditionError::Inspect(error)
        | CurrentFieldAdditionError::Native(EditError::Inspect(error)) => inspection_failure(error),
        _ => None,
    }
}

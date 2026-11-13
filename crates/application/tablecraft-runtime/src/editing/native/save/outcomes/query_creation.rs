//! TCQC preserves its typed failure owners through the new query-created axis.
use super::{SaveFailureKind, inspection_failure, intent_failure, qualification_failure};
#[cfg(feature = "ace16-create")]
use tablecraft_ace::QueryCreationError;
use tablecraft_ace::QueryCreationIntentError;

#[cfg(feature = "ace16-create")]
pub(super) fn native(error: &QueryCreationError) -> Option<SaveFailureKind> {
    match error {
        QueryCreationError::Inspect(error) => inspection_failure(error),
        _ => None,
    }
}

pub(super) fn intent(error: &QueryCreationIntentError) -> Option<SaveFailureKind> {
    match error {
        QueryCreationIntentError::Inspect(error) => inspection_failure(error),
        QueryCreationIntentError::Binding(error) => intent_failure(error),
        #[cfg(feature = "ace16-create")]
        QueryCreationIntentError::Native(error) => native(error),
        QueryCreationIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

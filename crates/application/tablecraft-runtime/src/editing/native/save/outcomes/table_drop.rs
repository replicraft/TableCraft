//! Table DROP retains resource/control classification through its typed error layers.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{TableDropError, TableDropIntentError};

pub(super) fn candidate(error: &TableDropError) -> Option<SaveFailureKind> {
    match error {
        TableDropError::Inspect(error) => inspection_failure(error),
        _ => None,
    }
}

pub(super) fn intent(error: &TableDropIntentError) -> Option<SaveFailureKind> {
    match error {
        TableDropIntentError::Inspect(error) => inspection_failure(error),
        TableDropIntentError::Binding(error) => intent_failure(error),
        TableDropIntentError::Drop(error) => candidate(error),
        TableDropIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

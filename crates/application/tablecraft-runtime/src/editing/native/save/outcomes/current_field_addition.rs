//! Current source and original request retain typed cancellation/resource facts.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{
    CurrentFieldAdditionError, CurrentFieldAdditionIntentError,
    DeclaredFieldAdditionIntentError as DeclaredError, EditError,
};

pub(super) fn intent(error: &CurrentFieldAdditionIntentError) -> Option<SaveFailureKind> {
    match error {
        CurrentFieldAdditionIntentError::Inspect(error) => inspection_failure(error),
        CurrentFieldAdditionIntentError::Binding(error) => intent_failure(error),
        CurrentFieldAdditionIntentError::Proof(error) => qualification_failure(error),
        CurrentFieldAdditionIntentError::Field(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn retained_typed(
    error: &tablecraft_ace::RetainedTypedFieldAdditionIntentError,
) -> Option<SaveFailureKind> {
    use tablecraft_ace::RetainedTypedFieldAdditionIntentError as Error;
    match error {
        Error::Inspect(error) => inspection_failure(error),
        #[cfg(feature = "ace16-create")]
        Error::Binding(error) => intent_failure(error),
        Error::Proof(error) => qualification_failure(error),
        Error::Field(error) => candidate(error),
        #[cfg(feature = "ace16-create")]
        Error::Declaration(tablecraft_ace::CreateDatabaseError::Inspect(error)) => {
            inspection_failure(error)
        }
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

pub(super) fn declared(error: &DeclaredError) -> Option<SaveFailureKind> {
    match error {
        DeclaredError::Inspect(error) => inspection_failure(error),
        #[cfg(feature = "ace16-create")]
        DeclaredError::Binding(error) => intent_failure(error),
        DeclaredError::Field(error) => candidate(error),
        DeclaredError::Proof(error) => qualification_failure(error),
        #[cfg(feature = "ace16-create")]
        DeclaredError::Declaration(tablecraft_ace::CreateDatabaseError::Inspect(error)) => {
            inspection_failure(error)
        }
        _ => None,
    }
}

#[cfg(all(test, feature = "ace16-create"))]
mod declared_tests;

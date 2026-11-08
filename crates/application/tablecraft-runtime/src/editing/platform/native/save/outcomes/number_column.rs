//! Typed Number-intent failures preserve cancellation/resource classifications.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{
    CreateDatabaseError, CreatedAddColumnError, CreatedDefaultZeroAddColumnError,
    DefaultZeroAddColumnIntentError, EditError,
};

pub(super) fn intent(error: &DefaultZeroAddColumnIntentError) -> Option<SaveFailureKind> {
    match error {
        DefaultZeroAddColumnIntentError::Inspect(error) => inspection_failure(error),
        DefaultZeroAddColumnIntentError::Binding(error) => intent_failure(error),
        DefaultZeroAddColumnIntentError::Proof(error) => qualification_failure(error),
        DefaultZeroAddColumnIntentError::Column(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &CreatedDefaultZeroAddColumnError) -> Option<SaveFailureKind> {
    match error {
        CreatedDefaultZeroAddColumnError::Inspect(error)
        | CreatedDefaultZeroAddColumnError::Declaration(CreateDatabaseError::Inspect(error))
        | CreatedDefaultZeroAddColumnError::Placement(EditError::Inspect(error))
        | CreatedDefaultZeroAddColumnError::Created(CreatedAddColumnError::Inspect(error))
        | CreatedDefaultZeroAddColumnError::Created(CreatedAddColumnError::Declaration(
            CreateDatabaseError::Inspect(error),
        )) => inspection_failure(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{candidate, intent};
    use crate::editing::SaveFailureKind;
    use tablecraft_ace::{
        CreateDatabaseError, CreatedAddColumnError, CreatedDefaultZeroAddColumnError as Candidate,
        DefaultZeroAddColumnIntentError as Intent, EditError, InspectError,
    };

    #[test]
    fn cancellation_survives_every_new_candidate_resource_wrapper() {
        let errors = [
            Candidate::Inspect(InspectError::Cancelled),
            Candidate::Declaration(CreateDatabaseError::Inspect(InspectError::Cancelled)),
            Candidate::Placement(EditError::Inspect(InspectError::Cancelled)),
            Candidate::Created(CreatedAddColumnError::Inspect(InspectError::Cancelled)),
            Candidate::Created(CreatedAddColumnError::Declaration(
                CreateDatabaseError::Inspect(InspectError::Cancelled),
            )),
        ];
        for error in errors {
            assert_eq!(candidate(&error), Some(SaveFailureKind::Cancelled));
            assert_eq!(
                intent(&Intent::Column(error)),
                Some(SaveFailureKind::Cancelled)
            );
        }
    }

    #[test]
    fn allocation_is_resource_while_unsupported_source_is_not_misclassified() {
        let error = Intent::Column(Candidate::Placement(EditError::Inspect(
            InspectError::Allocation,
        )));
        assert_eq!(intent(&error), Some(SaveFailureKind::Resource));
        assert_eq!(intent(&Intent::Column(Candidate::Source)), None);
        assert_eq!(intent(&Intent::InvalidRecord), None);
    }
}

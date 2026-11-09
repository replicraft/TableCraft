//! Typed field declaration failures retain resource/cancellation classification.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{
    CreateDatabaseError, EditError, FieldAdditionError, FieldAdditionIntentError,
};

pub(super) fn intent(error: &FieldAdditionIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionIntentError::Inspect(error) => inspection_failure(error),
        FieldAdditionIntentError::Binding(error) => intent_failure(error),
        FieldAdditionIntentError::Proof(error) => qualification_failure(error),
        FieldAdditionIntentError::Field(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &FieldAdditionError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionError::Inspect(error)
        | FieldAdditionError::Declaration(CreateDatabaseError::Inspect(error))
        | FieldAdditionError::Placement(EditError::Inspect(error)) => inspection_failure(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_ace::{InspectError, NativeIntentError, NativeRecoveryQualificationError};

    #[test]
    fn every_typed_field_layer_keeps_cancel_and_resource_identity() {
        for resource in [false, true] {
            let error = || {
                if resource {
                    InspectError::Allocation
                } else {
                    InspectError::Cancelled
                }
            };
            let expected = Some(if resource {
                SaveFailureKind::Resource
            } else {
                SaveFailureKind::Cancelled
            });
            for wrapped in [
                FieldAdditionIntentError::Inspect(error()),
                FieldAdditionIntentError::Binding(NativeIntentError::Inspect(error())),
                FieldAdditionIntentError::Proof(NativeRecoveryQualificationError::Inspect(error())),
                FieldAdditionIntentError::Field(FieldAdditionError::Inspect(error())),
                FieldAdditionIntentError::Field(FieldAdditionError::Declaration(
                    CreateDatabaseError::Inspect(error()),
                )),
                FieldAdditionIntentError::Field(FieldAdditionError::Placement(EditError::Inspect(
                    error(),
                ))),
            ] {
                assert_eq!(intent(&wrapped), expected);
            }
        }
        assert_eq!(
            intent(&FieldAdditionIntentError::Field(FieldAdditionError::Source)),
            None
        );
        assert_eq!(intent(&FieldAdditionIntentError::InvalidRecord), None);
    }
}

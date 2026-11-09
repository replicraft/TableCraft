//! Preserve nested native cancellation and resource failures for TCAC6.
use super::{
    database_options_field_history, field_addition, inspection_failure, intent_failure,
    qualification_failure,
};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{EditError, FieldAdditionAfterOffError, FieldAdditionAfterOffIntentError};

pub(super) fn intent(error: &FieldAdditionAfterOffIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionAfterOffIntentError::Inspect(error) => inspection_failure(error),
        FieldAdditionAfterOffIntentError::Binding(error) => intent_failure(error),
        FieldAdditionAfterOffIntentError::Proof(error) => qualification_failure(error),
        FieldAdditionAfterOffIntentError::Field(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &FieldAdditionAfterOffError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionAfterOffError::Inspect(error)
        | FieldAdditionAfterOffError::Placement(EditError::Inspect(error)) => {
            inspection_failure(error)
        }
        FieldAdditionAfterOffError::SourceProof(error) => {
            database_options_field_history::candidate(error)
        }
        FieldAdditionAfterOffError::Field(error) => field_addition::candidate(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{
        FieldAdditionError, FieldHistoryDatabaseOptionsError, InspectError, NativeIntentError,
        NativeRecoveryQualificationError,
    };

    #[test]
    fn nested_after_off_failure_identity_survives_cause_and_capability_layers() {
        for resource in [false, true] {
            let inspect = || {
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
            for error in [
                FieldAdditionAfterOffIntentError::Inspect(inspect()),
                FieldAdditionAfterOffIntentError::Binding(NativeIntentError::Inspect(inspect())),
                FieldAdditionAfterOffIntentError::Proof(NativeRecoveryQualificationError::Inspect(
                    inspect(),
                )),
                FieldAdditionAfterOffIntentError::Field(FieldAdditionAfterOffError::Inspect(
                    inspect(),
                )),
                FieldAdditionAfterOffIntentError::Field(FieldAdditionAfterOffError::Placement(
                    EditError::Inspect(inspect()),
                )),
                FieldAdditionAfterOffIntentError::Field(FieldAdditionAfterOffError::Field(
                    FieldAdditionError::Placement(EditError::Inspect(inspect())),
                )),
                FieldAdditionAfterOffIntentError::Field(FieldAdditionAfterOffError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Placement(
                        EditError::Inspect(inspect()),
                    )),
                )),
            ] {
                assert_eq!(intent(&error), expected);
                let cause = Cause::FieldAdditionAfterOffIntent(error);
                assert_eq!(super::super::resource_failure(&cause), expected);
            }
        }
        assert_eq!(candidate(&FieldAdditionAfterOffError::Namespace), None);
    }
}

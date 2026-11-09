//! Preserve real history qualification, append and request error identity.
use super::{field_addition_after_off, inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{FieldAdditionAfterOffHistoryError, FieldAdditionAfterOffHistoryIntentError};

pub(super) fn intent(error: &FieldAdditionAfterOffHistoryIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionAfterOffHistoryIntentError::Inspect(error) => inspection_failure(error),
        FieldAdditionAfterOffHistoryIntentError::Binding(error) => intent_failure(error),
        FieldAdditionAfterOffHistoryIntentError::Proof(error) => qualification_failure(error),
        FieldAdditionAfterOffHistoryIntentError::Field(error) => candidate(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &FieldAdditionAfterOffHistoryError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionAfterOffHistoryError::Inspect(error) => inspection_failure(error),
        FieldAdditionAfterOffHistoryError::Field(error) => {
            field_addition_after_off::candidate(error)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{
        EditError, FieldAdditionAfterOffError, FieldAdditionError,
        FieldHistoryDatabaseOptionsError, InspectError, NativeIntentError,
        NativeRecoveryQualificationError,
    };

    #[test]
    fn nested_history_failure_kinds_survive_the_actual_adapter_cause() {
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
                FieldAdditionAfterOffHistoryIntentError::Inspect(inspect()),
                FieldAdditionAfterOffHistoryIntentError::Binding(NativeIntentError::Inspect(
                    inspect(),
                )),
                FieldAdditionAfterOffHistoryIntentError::Proof(
                    NativeRecoveryQualificationError::Inspect(inspect()),
                ),
                FieldAdditionAfterOffHistoryIntentError::Field(
                    FieldAdditionAfterOffHistoryError::Inspect(inspect()),
                ),
                FieldAdditionAfterOffHistoryIntentError::Field(
                    FieldAdditionAfterOffHistoryError::Field(
                        FieldAdditionAfterOffError::Placement(EditError::Inspect(inspect())),
                    ),
                ),
                FieldAdditionAfterOffHistoryIntentError::Field(
                    FieldAdditionAfterOffHistoryError::Field(
                        FieldAdditionAfterOffError::SourceProof(
                            FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Placement(
                                EditError::Inspect(inspect()),
                            )),
                        ),
                    ),
                ),
            ] {
                assert_eq!(intent(&error), expected);
                assert_eq!(
                    super::super::resource_failure(&Cause::FieldAdditionAfterOffHistoryIntent(
                        error
                    )),
                    expected
                );
            }
        }
        assert_eq!(candidate(&FieldAdditionAfterOffHistoryError::Source), None);
        assert_eq!(
            candidate(&FieldAdditionAfterOffHistoryError::Preservation),
            None
        );
    }
}

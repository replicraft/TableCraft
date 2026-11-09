//! Exact new options failures retain resource/cancellation identity through layers.
use super::{field_addition, inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{
    CreateDatabaseError, DatabaseOptionsSourceError, EditError, FieldHistoryDatabaseOptionsError,
    FieldHistoryOptionsIntentError,
};

pub(super) fn intent(error: &FieldHistoryOptionsIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldHistoryOptionsIntentError::Inspect(error) => inspection_failure(error),
        FieldHistoryOptionsIntentError::Binding(error) => intent_failure(error),
        FieldHistoryOptionsIntentError::Options(error) => candidate(error),
        FieldHistoryOptionsIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

pub(super) fn candidate(error: &FieldHistoryDatabaseOptionsError) -> Option<SaveFailureKind> {
    match error {
        FieldHistoryDatabaseOptionsError::Inspect(error)
        | FieldHistoryDatabaseOptionsError::Create(CreateDatabaseError::Inspect(error))
        | FieldHistoryDatabaseOptionsError::Placement(EditError::Inspect(error)) => {
            inspection_failure(error)
        }
        FieldHistoryDatabaseOptionsError::Field(error) => field_addition::candidate(error),
        _ => None,
    }
}

pub(super) fn source(error: &DatabaseOptionsSourceError) -> Option<SaveFailureKind> {
    match error {
        DatabaseOptionsSourceError::Inspect(error) => inspection_failure(error),
        DatabaseOptionsSourceError::Creator(error) => super::database_options_failure(error),
        DatabaseOptionsSourceError::FieldHistory(error) => candidate(error),
        DatabaseOptionsSourceError::Storage(EditError::Inspect(error)) => inspection_failure(error),
        DatabaseOptionsSourceError::Source | DatabaseOptionsSourceError::Storage(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::platform::save::outcomes::resource_failure;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{
        FieldAdditionError, InspectError, NativeIntentError, NativeRecoveryQualificationError,
    };

    #[test]
    fn field_history_options_layers_and_actual_cause_keep_cancel_and_resource_kinds() {
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
            let layers = [
                FieldHistoryOptionsIntentError::Inspect(inspect()),
                FieldHistoryOptionsIntentError::Binding(NativeIntentError::Inspect(inspect())),
                FieldHistoryOptionsIntentError::Proof(NativeRecoveryQualificationError::Inspect(
                    inspect(),
                )),
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Inspect(
                    inspect(),
                )),
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Create(
                    CreateDatabaseError::Inspect(inspect()),
                )),
                FieldHistoryOptionsIntentError::Options(
                    FieldHistoryDatabaseOptionsError::Placement(EditError::Inspect(inspect())),
                ),
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Field(
                    FieldAdditionError::Inspect(inspect()),
                )),
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Field(
                    FieldAdditionError::Declaration(CreateDatabaseError::Inspect(inspect())),
                )),
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Field(
                    FieldAdditionError::Placement(EditError::Inspect(inspect())),
                )),
            ];
            for layer in layers {
                assert_eq!(resource_failure(&layer), expected);
                assert_eq!(
                    resource_failure(&Cause::FieldHistoryDatabaseOptionsIntent(layer)),
                    expected
                );
            }
            assert_eq!(
                resource_failure(&DatabaseOptionsSourceError::FieldHistory(
                    FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Inspect(inspect()))
                )),
                expected
            );
        }
        assert_eq!(
            resource_failure(&Cause::FieldHistoryDatabaseOptionsIntent(
                FieldHistoryOptionsIntentError::Options(FieldHistoryDatabaseOptionsError::Source)
            )),
            None
        );
        assert_eq!(resource_failure(&DatabaseOptionsSourceError::Source), None);
    }
}

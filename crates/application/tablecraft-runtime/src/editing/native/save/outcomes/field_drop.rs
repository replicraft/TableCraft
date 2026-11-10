//! DROP controls keep their original typed layer through retained recovery reports.
#[cfg(feature = "ace16-create")]
use super::database_options_field_history;
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::FieldDropIntentError;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{CreateDatabaseError, CreatedAddColumnError, EditError, FieldDropError};

#[cfg(feature = "ace16-create")]
pub(super) fn candidate(error: &FieldDropError) -> Option<SaveFailureKind> {
    match error {
        FieldDropError::Inspect(error)
        | FieldDropError::Metadata(CreatedAddColumnError::Inspect(error))
        | FieldDropError::Metadata(CreatedAddColumnError::Declaration(
            CreateDatabaseError::Inspect(error),
        ))
        | FieldDropError::Declaration(CreateDatabaseError::Inspect(error))
        | FieldDropError::Placement(EditError::Inspect(error)) => inspection_failure(error),
        FieldDropError::SourceProof(error) => database_options_field_history::candidate(error),
        _ => None,
    }
}

pub(super) fn intent(error: &FieldDropIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldDropIntentError::Inspect(error) => inspection_failure(error),
        FieldDropIntentError::Binding(error) => intent_failure(error),
        #[cfg(feature = "ace16-create")]
        FieldDropIntentError::Drop(error) => candidate(error),
        FieldDropIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

#[cfg(all(test, feature = "ace16-create"))]
mod tests {
    use super::super::resource_failure;
    use super::*;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{
        FieldAdditionError, FieldHistoryDatabaseOptionsError, InspectError, NativeIntentError,
        NativeRecoveryQualificationError,
    };

    #[test]
    fn every_drop_control_layer_and_actual_adapter_cause_preserve_cancel_and_resource_facts() {
        for resource in [false, true] {
            let failure = || {
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
                FieldDropIntentError::Inspect(failure()),
                FieldDropIntentError::Binding(NativeIntentError::Inspect(failure())),
                FieldDropIntentError::Proof(NativeRecoveryQualificationError::Inspect(failure())),
                FieldDropIntentError::Drop(FieldDropError::Inspect(failure())),
                FieldDropIntentError::Drop(FieldDropError::Metadata(
                    CreatedAddColumnError::Inspect(failure()),
                )),
                FieldDropIntentError::Drop(FieldDropError::Metadata(
                    CreatedAddColumnError::Declaration(CreateDatabaseError::Inspect(failure())),
                )),
                FieldDropIntentError::Drop(FieldDropError::Declaration(
                    CreateDatabaseError::Inspect(failure()),
                )),
                FieldDropIntentError::Drop(FieldDropError::Placement(
                    EditError::Inspect(failure()),
                )),
                FieldDropIntentError::Drop(FieldDropError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Inspect(failure()),
                )),
                FieldDropIntentError::Drop(FieldDropError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Placement(
                        EditError::Inspect(failure()),
                    )),
                )),
            ] {
                assert_eq!(resource_failure(&wrapped), expected);
                assert_eq!(resource_failure(&Cause::FieldDropIntent(wrapped)), expected);
            }
        }
        for refusal in [
            FieldDropIntentError::InvalidRecord,
            FieldDropIntentError::IdentityMismatch,
            FieldDropIntentError::Drop(FieldDropError::Source),
            FieldDropIntentError::Drop(FieldDropError::Preservation),
        ] {
            assert_eq!(resource_failure(&Cause::FieldDropIntent(refusal)), None);
        }
    }
}

//! Rename control failures retain their real typed nesting and outcome identity.
#[cfg(feature = "ace16-create")]
use super::database_options_field_history;
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::FieldRenameError;
use tablecraft_ace::FieldRenameIntentError;

#[cfg(feature = "ace16-create")]
pub(super) fn candidate(error: &FieldRenameError) -> Option<SaveFailureKind> {
    match error {
        FieldRenameError::Inspect(error) => inspection_failure(error),
        FieldRenameError::SourceProof(error) => database_options_field_history::candidate(error),
        _ => None,
    }
}

pub(super) fn intent(error: &FieldRenameIntentError) -> Option<SaveFailureKind> {
    match error {
        FieldRenameIntentError::Inspect(error) => inspection_failure(error),
        FieldRenameIntentError::Binding(error) => intent_failure(error),
        #[cfg(feature = "ace16-create")]
        FieldRenameIntentError::Rename(error) => candidate(error),
        FieldRenameIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

#[cfg(all(test, feature = "ace16-create"))]
mod tests {
    use super::*;
    use crate::editing::platform::save::outcomes::resource_failure;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{
        EditError, FieldAdditionError, FieldHistoryDatabaseOptionsError, InspectError,
        NativeIntentError, NativeRecoveryQualificationError,
    };

    #[test]
    fn native_source_reconstruction_and_actual_adapter_cause_preserve_control_failures() {
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
                FieldRenameIntentError::Inspect(failure()),
                FieldRenameIntentError::Binding(NativeIntentError::Inspect(failure())),
                FieldRenameIntentError::Proof(NativeRecoveryQualificationError::Inspect(failure())),
                FieldRenameIntentError::Rename(FieldRenameError::Inspect(failure())),
                FieldRenameIntentError::Rename(FieldRenameError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Inspect(failure()),
                )),
                FieldRenameIntentError::Rename(FieldRenameError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Placement(
                        EditError::Inspect(failure()),
                    )),
                )),
            ] {
                assert_eq!(resource_failure(&wrapped), expected);
                assert_eq!(
                    resource_failure(&Cause::FieldRenameIntent(wrapped)),
                    expected
                );
            }
        }
        assert_eq!(
            resource_failure(&Cause::FieldRenameIntent(FieldRenameIntentError::Rename(
                FieldRenameError::Source
            ))),
            None
        );
        assert_eq!(
            resource_failure(&FieldRenameIntentError::IdentityMismatch),
            None
        );
    }
}

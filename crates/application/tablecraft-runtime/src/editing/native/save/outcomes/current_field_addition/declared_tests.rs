//! Transparent original layers must not erase cancellation or resource identity.
use super::super::resource_failure;
use crate::editing::SaveFailureKind;
use tablecraft_access::editing::offline::Cause;
use tablecraft_ace::{
    CreateDatabaseError, CurrentFieldAdditionError, DeclaredFieldAdditionIntentError as Error,
    EditError, InspectError, NativeIntentError, NativeRecoveryQualificationError,
};

#[test]
fn declared_original_layers_keep_cancel_and_resource_kind_through_actual_host_cause() {
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
            Error::Inspect(error()),
            Error::Binding(NativeIntentError::Inspect(error())),
            Error::Declaration(CreateDatabaseError::Inspect(error())),
            Error::Field(CurrentFieldAdditionError::Inspect(error())),
            Error::Field(CurrentFieldAdditionError::Native(EditError::Inspect(
                error(),
            ))),
            Error::Proof(NativeRecoveryQualificationError::Inspect(error())),
        ] {
            assert_eq!(
                resource_failure(&Cause::DeclaredFieldAdditionIntent(wrapped)),
                expected
            );
        }
    }
    assert_eq!(
        resource_failure(&Cause::DeclaredFieldAdditionIntent(Error::Field(
            CurrentFieldAdditionError::Source
        ))),
        None
    );
}

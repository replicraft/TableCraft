//! Actual host Cause handling cannot lose a transparent leaf failure.
use super::super::resource_failure;
use crate::editing::SaveFailureKind;
use tablecraft_access::editing::offline::Cause;
use tablecraft_ace::{
    EditError, FreshIndexCreateIntentError as Error, InspectError, NativeIntentError,
    NativeRecoveryQualificationError,
};
#[test]
fn fresh_original_wrappers_preserve_actual_cancel_and_resource_kinds() {
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
            Error::Edit(EditError::Inspect(error())),
            Error::Proof(NativeRecoveryQualificationError::Inspect(error())),
        ] {
            assert_eq!(
                resource_failure(&Cause::FreshIndexCreationIntent(wrapped)),
                expected
            );
        }
    }
    assert_eq!(
        resource_failure(&Cause::FreshIndexCreationIntent(Error::UnsupportedSource)),
        None
    );
}

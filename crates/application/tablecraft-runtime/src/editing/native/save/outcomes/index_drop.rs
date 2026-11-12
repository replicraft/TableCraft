//! Inspect typed DROP failures before transparent error traversal loses the owner.
use super::{inspection_failure, intent_failure, qualification_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::{EditError, IndexDropIntentError};

pub(super) fn intent(error: &IndexDropIntentError) -> Option<SaveFailureKind> {
    match error {
        IndexDropIntentError::Inspect(error) => inspection_failure(error),
        IndexDropIntentError::Edit(EditError::Inspect(error)) => inspection_failure(error),
        IndexDropIntentError::Proof(error) => qualification_failure(error),
        IndexDropIntentError::Binding(error) => intent_failure(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::resource_failure;
    use super::*;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{InspectError, NativeIntentError, NativeRecoveryQualificationError};
    use tablecraft_core::limits::{LimitError, Resource};

    type Wrap = fn(InspectError) -> IndexDropIntentError;
    type Sample = (fn() -> InspectError, SaveFailureKind);

    #[test]
    fn direct_native_and_access_owners_keep_cancel_and_resource_classification() {
        let wraps: [Wrap; 4] = [
            IndexDropIntentError::Inspect,
            |error| IndexDropIntentError::Edit(EditError::Inspect(error)),
            |error| IndexDropIntentError::Binding(NativeIntentError::Inspect(error)),
            |error| IndexDropIntentError::Proof(NativeRecoveryQualificationError::Inspect(error)),
        ];
        let samples: [Sample; 3] = [
            (|| InspectError::Cancelled, SaveFailureKind::Cancelled),
            (
                || {
                    InspectError::Limit(LimitError {
                        resource: Resource::Steps,
                        remaining: 0,
                    })
                },
                SaveFailureKind::Resource,
            ),
            (|| InspectError::Allocation, SaveFailureKind::Resource),
        ];
        for wrap in wraps {
            for (make, expected) in samples {
                let error = wrap(make());
                assert_eq!(resource_failure(&error), Some(expected));
                assert_eq!(
                    resource_failure(&Cause::IndexDropIntent(error)),
                    Some(expected)
                );
            }
        }
        assert_eq!(intent(&IndexDropIntentError::BeforeMismatch), None);
    }
}

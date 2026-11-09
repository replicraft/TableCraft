//! TCQE0002 preserves typed resource/cancellation facts through transparent errors.
use super::{SaveFailureKind, inspection_failure, intent_failure, qualification_failure};
use tablecraft_ace::{QueryExpressionEditsError, QueryExpressionEditsIntentError};

pub(super) fn expression(error: &QueryExpressionEditsError) -> Option<SaveFailureKind> {
    match error {
        QueryExpressionEditsError::Inspect(error) => inspection_failure(error),
        _ => None,
    }
}

pub(super) fn intent(error: &QueryExpressionEditsIntentError) -> Option<SaveFailureKind> {
    match error {
        QueryExpressionEditsIntentError::Inspect(error) => inspection_failure(error),
        QueryExpressionEditsIntentError::Binding(error) => intent_failure(error),
        QueryExpressionEditsIntentError::Expression(error) => expression(error),
        QueryExpressionEditsIntentError::Proof(error) => qualification_failure(error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::platform::native::save::outcomes::resource_failure;
    use tablecraft_access::editing::offline::Cause;
    use tablecraft_ace::{InspectError, NativeIntentError, NativeRecoveryQualificationError};
    use tablecraft_core::limits::{LimitError, Resource};

    type Wrapper = fn(InspectError) -> QueryExpressionEditsIntentError;

    #[test]
    fn every_fragment_error_wrapper_retains_resource_and_cancel_classification() {
        let wrappers: [Wrapper; 4] = [
            QueryExpressionEditsIntentError::Inspect,
            |error| QueryExpressionEditsIntentError::Binding(NativeIntentError::Inspect(error)),
            |error| {
                QueryExpressionEditsIntentError::Expression(QueryExpressionEditsError::Inspect(
                    error,
                ))
            },
            |error| {
                QueryExpressionEditsIntentError::Proof(NativeRecoveryQualificationError::Inspect(
                    error,
                ))
            },
        ];
        for wrap in wrappers {
            assert_eq!(
                resource_failure(&Cause::QueryExpressionEditsIntent(wrap(
                    InspectError::Cancelled,
                ))),
                Some(SaveFailureKind::Cancelled)
            );
            assert_eq!(
                resource_failure(&Cause::QueryExpressionEditsIntent(wrap(
                    InspectError::Limit(LimitError {
                        resource: Resource::Bytes,
                        remaining: 0
                    }),
                ))),
                Some(SaveFailureKind::Resource)
            );
        }
        assert_eq!(resource_failure(&QueryExpressionEditsError::Source), None);
        assert_eq!(
            resource_failure(&QueryExpressionEditsIntentError::InvalidRecord),
            None
        );
    }
}

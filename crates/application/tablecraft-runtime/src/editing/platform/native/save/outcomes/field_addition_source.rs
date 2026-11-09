//! Source-only refusal is not cancellation, allocation or malformed inspection.
use super::{field_addition, field_addition_after_off, inspection_failure};
use crate::editing::SaveFailureKind;
use tablecraft_ace::FieldAdditionSourceError;

pub(super) fn failure(error: &FieldAdditionSourceError) -> Option<SaveFailureKind> {
    match error {
        FieldAdditionSourceError::Inspect(error) => inspection_failure(error),
        FieldAdditionSourceError::Created(error) => field_addition::candidate(error),
        FieldAdditionSourceError::AfterOff(error) => field_addition_after_off::candidate(error),
        FieldAdditionSourceError::Source => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_ace::{
        EditError, FieldAdditionAfterOffError, FieldAdditionError,
        FieldHistoryDatabaseOptionsError, InspectError,
    };

    #[test]
    fn actual_source_layers_preserve_resource_and_cancellation_classification() {
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
                FieldAdditionSourceError::Inspect(inspect()),
                FieldAdditionSourceError::Created(FieldAdditionError::Placement(
                    EditError::Inspect(inspect()),
                )),
                FieldAdditionSourceError::AfterOff(FieldAdditionAfterOffError::Placement(
                    EditError::Inspect(inspect()),
                )),
                FieldAdditionSourceError::AfterOff(FieldAdditionAfterOffError::SourceProof(
                    FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Placement(
                        EditError::Inspect(inspect()),
                    )),
                )),
            ] {
                assert_eq!(failure(&error), expected);
                assert_eq!(super::super::resource_failure(&error), expected);
            }
        }
        assert_eq!(failure(&FieldAdditionSourceError::Source), None);
    }
}

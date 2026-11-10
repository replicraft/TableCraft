//! Native source inspection holds no save lease or target DROP AFTER image.
mod review;
pub(in crate::editing) use review::{State, review};

use crate::{Error, controls::SCOPE, document::Document};
use tablecraft_ace::{
    CreatedAddColumnError, EditError as NativeEditError, FieldAdditionError, FieldDropError,
    FieldHistoryDatabaseOptionsError, NativeImage,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) fn source_eligible(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    if !document.source.editable_native() {
        return Ok(false);
    }
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image = NativeImage::capture_shared(
        document.edits.image_source.clone(),
        SCOPE,
        budget,
        cancelled,
    )?;
    let identity =
        source_result(image.inspect_field_drop_source(table, column, budget, cancelled))?;
    let Some(identity) = identity else {
        return Ok(false);
    };
    if identity.column_number() != native_number {
        return Err(Error::StaleFieldDropTarget);
    }
    Ok(true)
}

fn source_result(
    result: Result<tablecraft_ace::NativeFieldDropIdentity, FieldDropError>,
) -> Result<Option<tablecraft_ace::NativeFieldDropIdentity>, Error> {
    match result {
        Ok(identity) => Ok(Some(identity)),
        Err(FieldDropError::Source)
        | Err(FieldDropError::Placement(NativeEditError::Unsupported(_)))
        | Err(FieldDropError::Metadata(CreatedAddColumnError::Source))
        | Err(FieldDropError::SourceProof(FieldHistoryDatabaseOptionsError::Source))
        | Err(FieldDropError::SourceProof(FieldHistoryDatabaseOptionsError::Field(
            FieldAdditionError::Source,
        ))) => Ok(None),
        Err(error) => Err(Error::FieldDropSource(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::source_result;
    use crate::Error;
    use tablecraft_ace::{FieldDropError, FieldHistoryDatabaseOptionsError, InspectError};

    #[test]
    fn malformed_cancelled_and_resource_failures_never_become_unavailable() {
        for expected in [
            InspectError::Malformed,
            InspectError::Cancelled,
            InspectError::Allocation,
        ] {
            let expected_kind = std::mem::discriminant(&expected);
            let result = source_result(Err(FieldDropError::SourceProof(
                FieldHistoryDatabaseOptionsError::Inspect(expected),
            )));
            let Err(Error::FieldDropSource(FieldDropError::SourceProof(
                FieldHistoryDatabaseOptionsError::Inspect(actual),
            ))) = result
            else {
                panic!("inspection failure must retain its typed source error");
            };
            assert_eq!(std::mem::discriminant(&actual), expected_kind);
        }
        let mut work = tablecraft_core::limits::ResourceBudget::new(0, 0, 0);
        let limit = work
            .charge(tablecraft_core::limits::Resource::Steps, 1)
            .unwrap_err();
        assert!(matches!(
            source_result(Err(FieldDropError::Inspect(limit.into()))),
            Err(Error::FieldDropSource(FieldDropError::Inspect(
                InspectError::Limit(_)
            )))
        ));
        assert!(
            source_result(Err(FieldDropError::Source))
                .unwrap()
                .is_none()
        );
    }
}

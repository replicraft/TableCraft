//! Native source inspection consumes neither a save lease nor an AFTER image.
mod review;
pub(in crate::editing) use review::{State, review};

use crate::{Error, controls::SCOPE, document::Document};
use tablecraft_ace::{
    FieldAdditionError, FieldHistoryDatabaseOptionsError, FieldRenameError, NativeImage,
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) fn source_width(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<usize>, Error> {
    if !document.source.editable_native() {
        return Ok(None);
    }
    let image = NativeImage::capture_shared(
        // NativeImage owns the checked source while Document keeps using it; this shares
        // immutable bytes instead of copying the database image.
        document.edits.image_source.clone(),
        SCOPE,
        budget,
        cancelled,
    )?;
    let identity = source_result(
        image.inspect_same_width_field_rename_source(table, column, budget, cancelled),
    )?;
    let Some(identity) = identity else {
        return Ok(None);
    };
    if identity.column_number() != native_number {
        return Err(Error::StaleFieldRenameTarget);
    }
    Ok(Some(identity.name_utf16_units()))
}

fn source_result(
    result: Result<tablecraft_ace::NativeFieldRenameIdentity, FieldRenameError>,
) -> Result<Option<tablecraft_ace::NativeFieldRenameIdentity>, Error> {
    match result {
        Ok(identity) => Ok(Some(identity)),
        Err(FieldRenameError::Source)
        | Err(FieldRenameError::SourceProof(FieldHistoryDatabaseOptionsError::Source))
        | Err(FieldRenameError::SourceProof(FieldHistoryDatabaseOptionsError::Field(
            FieldAdditionError::Source,
        ))) => Ok(None),
        Err(error) => Err(Error::FieldRenameSource(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::source_result;
    use crate::Error;
    use tablecraft_ace::{FieldHistoryDatabaseOptionsError, FieldRenameError, InspectError};

    #[test]
    fn malformed_cancelled_and_resource_failures_never_become_unavailable() {
        for expected in [
            InspectError::Malformed,
            InspectError::Cancelled,
            InspectError::Allocation,
        ] {
            let expected_kind = std::mem::discriminant(&expected);
            let result = source_result(Err(FieldRenameError::SourceProof(
                FieldHistoryDatabaseOptionsError::Inspect(expected),
            )));
            let Err(Error::FieldRenameSource(FieldRenameError::SourceProof(
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
            source_result(Err(FieldRenameError::Inspect(limit.into()))),
            Err(Error::FieldRenameSource(FieldRenameError::Inspect(
                InspectError::Limit(_)
            )))
        ));
        assert!(
            source_result(Err(FieldRenameError::Source))
                .unwrap()
                .is_none()
        );
    }
}

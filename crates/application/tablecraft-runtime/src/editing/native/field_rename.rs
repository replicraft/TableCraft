//! Native source inspection consumes neither a save lease nor an AFTER image.
mod review;
pub(in crate::editing) use review::{State, review};

use crate::{Error, controls::SCOPE, document::Document};
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::{
    FieldAdditionError, FieldHistoryDatabaseOptionsError, FieldRenameError,
    FittedFieldRenameSource, NativeImage,
};
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) fn source_profile(
    document: &mut Document,
    table: &str,
    column: &str,
    native_number: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<FieldRenameNameProfile>, Error> {
    if !document.source.editable_native() {
        return Ok(None);
    }
    // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this shares
    // immutable bytes instead of copying the database image.
    let image = NativeJournalEditor::capture_shared_image(
        document.edits.image_source.clone(),
        SCOPE,
        table,
        budget,
        cancelled,
    )?;
    // Original metadata selects the source language before inspection. A failed
    // source proof is never retried through another family.
    let identity = source_result(
        inspect_source(&image, table, column, budget, cancelled).map(|source| source.identity()),
    )?;
    let Some(identity) = identity else {
        return Ok(None);
    };
    if identity.column_number() != native_number {
        return Err(Error::StaleFieldRenameTarget);
    }
    Ok(Some(FieldRenameNameProfile::FittedAscii {
        original_units: identity.name_utf16_units(),
    }))
}

fn inspect_source(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FittedFieldRenameSource, FieldRenameError> {
    if image.has_retained_table_source(table, budget, cancelled)? {
        if image
            .select_native_retained_typed_field_rename_source(table, column, budget, cancelled)?
        {
            image.inspect_retained_typed_field_rename_source(table, column, budget, cancelled)
        } else {
            image.inspect_retained_schema_field_rename_source(table, column, budget, cancelled)
        }
    } else if image.select_native_retained_field_rename_source(table, column, budget, cancelled)? {
        image.inspect_retained_field_rename_source(table, column, budget, cancelled)
    } else {
        image.inspect_current_field_rename_source(table, column, budget, cancelled)
    }
}

fn source_result(
    result: Result<tablecraft_ace::NativeFieldRenameIdentity, FieldRenameError>,
) -> Result<Option<tablecraft_ace::NativeFieldRenameIdentity>, Error> {
    match result {
        Ok(identity) => Ok(Some(identity)),
        Err(FieldRenameError::Source)
        | Err(FieldRenameError::RowSource(tablecraft_ace::EditError::Unsupported(_)))
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
    use tablecraft_ace::{
        EditError, FieldHistoryDatabaseOptionsError, FieldRenameError, InspectError, WriteBlocker,
    };

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

    #[test]
    fn only_unsupported_populated_row_profiles_become_unavailable() {
        assert!(
            source_result(Err(FieldRenameError::RowSource(EditError::Unsupported(
                WriteBlocker::ConstraintCoverage,
            ))))
            .unwrap()
            .is_none()
        );
        for error in [
            EditError::Conflict,
            EditError::Preservation,
            EditError::Inspect(InspectError::Cancelled),
            EditError::Inspect(InspectError::Malformed),
        ] {
            let expected_kind = std::mem::discriminant(&error);
            let Err(Error::FieldRenameSource(FieldRenameError::RowSource(actual))) =
                source_result(Err(FieldRenameError::RowSource(error)))
            else {
                panic!("populated row failures must preserve their typed source error");
            };
            assert_eq!(std::mem::discriminant(&actual), expected_kind);
        }
    }
}

//! Positive source proofs without a speculative name or candidate allocation.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::FieldAdditionAvailability,
};
use std::num::NonZeroU8;
use std::sync::Arc;
use tablecraft_access::editing::journal::NativeJournalEditor;
use tablecraft_ace::{
    CurrentFieldAdditionError, EditError as NativeEditError, FieldAdditionSourceAvailability,
    FieldAdditionSourceError,
};
use tablecraft_builders::table::{
    DraftNumberSize, TypedFieldAdditionAvailability, TypedFieldAdditionDeclaration,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(in crate::editing) fn inspect(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.inspect_field_addition_source").entered();
    check(cancelled)?;
    let (available, typed) = if document.source.editable_native() {
        let mut work = budget();
        // NO_CLONE: NativeImage owns the checked source while Document keeps using it; this
        // shares immutable bytes instead of copying the database image.
        let image = NativeJournalEditor::capture_shared_image(
            document.edits.image_source.clone(),
            SCOPE,
            table,
            &mut work,
            cancelled,
        )?;
        let available = source_result(
            image
                .inspect_field_addition_source(table, &mut work, cancelled)
                .map_err(Error::from),
        )?;
        let typed = typed_sources(&image, table, &mut work, cancelled)?;
        (available, typed)
    } else {
        (None, Arc::from([]))
    };

    let mut output = Presentation::new(cancelled);
    let availability = FieldAdditionAvailability {
        document_revision: document.revision,
        table: output.text(table)?,
        number_available: available
            .as_ref()
            .is_some_and(FieldAdditionSourceAvailability::number_available),
        typed_declarations: typed,
        short_text_available: available
            .as_ref()
            .is_some_and(FieldAdditionSourceAvailability::short_text_available),
    };
    check(cancelled)?;
    Ok(Response::FieldAdditionAvailability { availability })
}

fn typed_sources(
    image: &tablecraft_ace::NativeImage,
    table: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Arc<[TypedFieldAdditionAvailability]>, Error> {
    if !image.has_retained_table_source(table, work, cancelled)? {
        return Ok(Arc::from([]));
    }
    let fixed = |kind| {
        TypedFieldAdditionDeclaration::fixed(kind)
            .map_err(|_| crate::editing::EditError::InvalidInput)
    };
    let decimal = DraftNumberSize::initial_decimal_constraints();
    let configurations = [
        fixed(ScalarType::Boolean)?,
        fixed(ScalarType::Byte)?,
        fixed(ScalarType::Int16)?,
        fixed(ScalarType::Int32)?,
        fixed(ScalarType::Int64)?,
        fixed(ScalarType::Currency)?,
        fixed(ScalarType::Single)?,
        fixed(ScalarType::Double)?,
        fixed(ScalarType::DateTime)?,
        fixed(ScalarType::Guid)?,
        TypedFieldAdditionDeclaration::decimal(decimal.precision(), decimal.scale())
            .map_err(|_| crate::editing::EditError::InvalidInput)?,
        TypedFieldAdditionDeclaration::short_text(NonZeroU8::MAX, true),
    ];
    work.charge_allocation(
        configurations.len() as u64,
        size_of::<TypedFieldAdditionAvailability>() as u64,
    )?;
    let mut eligible = Vec::new();
    eligible.try_reserve_exact(configurations.len())?;
    for declaration in configurations {
        check(cancelled)?;
        let target = super::declaration::target(declaration, work, cancelled)?;
        match image.inspect_retained_typed_field_addition_source(table, target, work, cancelled) {
            Ok(facts) if facts.maximum_name_utf16_units() != 0 => {
                eligible.push(TypedFieldAdditionAvailability {
                    declaration,
                    maximum_name_utf16_units: facts.maximum_name_utf16_units(),
                })
            }
            Ok(_)
            | Err(CurrentFieldAdditionError::Source)
            | Err(CurrentFieldAdditionError::Native(NativeEditError::Unsupported(_))) => {}
            Err(error) => {
                return Err(Error::FieldAdditionSource(
                    FieldAdditionSourceError::Current(error),
                ));
            }
        }
    }
    work.charge_allocation(
        eligible.len() as u64,
        size_of::<TypedFieldAdditionAvailability>() as u64,
    )?;
    work.charge_allocation(2, size_of::<usize>() as u64)?;
    Ok(Arc::from(eligible))
}

fn source_result(
    result: Result<FieldAdditionSourceAvailability, Error>,
) -> Result<Option<FieldAdditionSourceAvailability>, Error> {
    match result {
        Ok(facts) => Ok(Some(facts)),
        Err(Error::FieldAdditionSource(FieldAdditionSourceError::Source))
        | Err(Error::FieldAdditionSource(FieldAdditionSourceError::Current(
            CurrentFieldAdditionError::Source
            | CurrentFieldAdditionError::Native(NativeEditError::Unsupported(_)),
        ))) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_ace::InspectError;

    #[test]
    fn only_selected_source_refusal_becomes_an_unavailable_fact() {
        assert!(
            source_result(Err(Error::FieldAdditionSource(
                FieldAdditionSourceError::Source
            )))
            .unwrap()
            .is_none()
        );
        for (error, expected) in [
            (InspectError::Cancelled, InspectError::Cancelled),
            (InspectError::Allocation, InspectError::Allocation),
            (InspectError::Malformed, InspectError::Malformed),
        ] {
            let actual = source_result(Err(Error::FieldAdditionSource(
                FieldAdditionSourceError::Inspect(error),
            )));
            assert!(
                matches!(actual, Err(Error::FieldAdditionSource(FieldAdditionSourceError::Inspect(observed))) if observed == expected)
            );
        }
    }
}

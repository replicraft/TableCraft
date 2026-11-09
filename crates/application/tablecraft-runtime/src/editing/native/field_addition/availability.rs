//! Positive source proofs without a speculative name or candidate allocation.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::FieldAdditionAvailability,
};
use tablecraft_ace::{FieldAdditionSourceAvailability, FieldAdditionSourceError, NativeImage};

pub(in crate::editing) fn inspect(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.inspect_field_addition_source").entered();
    check(cancelled)?;
    let available = if document.source.editable_native() {
        let mut work = budget();
        let image = NativeImage::capture_shared(
            // NativeImage owns the checked source while Document keeps using it; this shares
            // immutable bytes instead of copying the database image.
            document.edits.image_source.clone(),
            SCOPE,
            &mut work,
            cancelled,
        )?;
        source_result(
            image
                .inspect_field_addition_source(table, &mut work, cancelled)
                .map_err(Error::from),
        )?
    } else {
        None
    };
    let mut output = Presentation::new(cancelled);
    let availability = FieldAdditionAvailability {
        document_revision: document.revision,
        table: output.text(table)?,
        number_available: available
            .as_ref()
            .is_some_and(FieldAdditionSourceAvailability::number_available),
        short_text_available: available
            .as_ref()
            .is_some_and(FieldAdditionSourceAvailability::short_text_available),
    };
    check(cancelled)?;
    Ok(Response::FieldAdditionAvailability { availability })
}

fn source_result(
    result: Result<FieldAdditionSourceAvailability, Error>,
) -> Result<Option<FieldAdditionSourceAvailability>, Error> {
    match result {
        Ok(facts) => Ok(Some(facts)),
        Err(Error::FieldAdditionSource(FieldAdditionSourceError::Source)) => Ok(None),
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

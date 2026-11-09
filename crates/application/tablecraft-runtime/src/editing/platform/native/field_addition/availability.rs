//! Positive source proofs without a speculative name or candidate allocation.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::FieldAdditionAvailability,
};
use tablecraft_ace::{FieldAdditionError, FieldAdditionIntentError, NativeImage};

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
            document.edits.image_source.clone(),
            SCOPE,
            &mut work,
            cancelled,
        )?;
        // ACE selects the immutable source purpose before proof and shares
        // that complete source owner with the typed candidate. This qualifies
        // both closed field kinds without a second history reconstruction.
        source_result(
            image
                .check_created_field_addition_source(table, &mut work, cancelled)
                .map_err(|error| Error::FieldAddition(error.into())),
        )?
    } else {
        false
    };
    let mut output = Presentation::new(cancelled);
    let availability = FieldAdditionAvailability {
        document_revision: document.revision,
        table: output.text(table)?,
        number_available: available,
        short_text_available: available,
    };
    check(cancelled)?;
    Ok(Response::FieldAdditionAvailability { availability })
}

fn source_result(result: Result<(), Error>) -> Result<bool, Error> {
    match result {
        Ok(()) => Ok(true),
        Err(Error::FieldAddition(FieldAdditionIntentError::Field(FieldAdditionError::Source))) => {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::source_result;
    use crate::Error;
    use tablecraft_ace::{FieldAdditionError, FieldAdditionIntentError, InspectError};

    #[test]
    fn only_typed_source_refusal_becomes_an_unavailable_fact() {
        assert!(
            !source_result(Err(Error::FieldAddition(FieldAdditionIntentError::Field(
                FieldAdditionError::Source
            ))))
            .unwrap()
        );
        for (input, expected) in [
            (InspectError::Cancelled, InspectError::Cancelled),
            (InspectError::Allocation, InspectError::Allocation),
            (InspectError::Malformed, InspectError::Malformed),
        ] {
            let result = source_result(Err(Error::FieldAddition(FieldAdditionIntentError::Field(
                FieldAdditionError::Inspect(input),
            ))));
            let Err(Error::FieldAddition(FieldAdditionIntentError::Field(
                FieldAdditionError::Inspect(actual),
            ))) = result
            else {
                panic!("failed inspection must retain its typed error");
            };

            assert_eq!(actual, expected);
        }
    }
}

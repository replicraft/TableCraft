//! Complete canonical/property source proof, without a speculative target name.
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::NumberColumnAvailability,
};
use tablecraft_ace::{CreatedAddColumnError, CreatedDefaultZeroAddColumnError, NativeImage};

pub(in crate::editing) fn inspect(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let _span = tracing::debug_span!("application.inspect_number_column_source").entered();
    check(cancelled)?;
    let available = if document.source.editable_native() {
        let mut work = budget();
        let image = NativeImage::capture_shared(
            document.edits.image_source.clone(),
            SCOPE,
            &mut work,
            cancelled,
        )?;
        let checked = (|| {
            let external =
                image.number_column_uses_external_properties(table, &mut work, cancelled)?;
            if external {
                image.check_repeated_default_zero_add_column_source(table, &mut work, cancelled)
            } else {
                image.check_created_default_zero_add_column_source(table, &mut work, cancelled)
            }
        })();
        match checked {
            Ok(()) => true,
            Err(error) => source_refusal(error)?,
        }
    } else {
        false
    };
    let mut output = Presentation::new(cancelled);
    let availability = NumberColumnAvailability {
        document_revision: document.revision,
        table: output.text(table)?,
        available,
    };
    check(cancelled)?;
    Ok(Response::NumberColumnAvailability { availability })
}

fn source_refusal(error: CreatedDefaultZeroAddColumnError) -> Result<bool, Error> {
    match error {
        CreatedDefaultZeroAddColumnError::Source
        | CreatedDefaultZeroAddColumnError::Created(CreatedAddColumnError::Source) => Ok(false),
        error => Err(Error::DefaultZeroAddColumn(error.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::source_refusal;
    use crate::Error;
    use tablecraft_ace::{
        CreatedAddColumnError, CreatedDefaultZeroAddColumnError as Candidate,
        DefaultZeroAddColumnIntentError as Intent, InspectError,
    };

    #[test]
    fn explicit_source_refusals_disable_but_failed_inspection_is_not_hidden() {
        for refusal in [
            Candidate::Source,
            Candidate::Created(CreatedAddColumnError::Source),
        ] {
            assert!(!source_refusal(refusal).unwrap());
        }
        for (error, expected) in [
            (InspectError::Cancelled, InspectError::Cancelled),
            (InspectError::Allocation, InspectError::Allocation),
            (InspectError::Malformed, InspectError::Malformed),
        ] {
            assert!(matches!(source_refusal(Candidate::Inspect(error)),
                Err(Error::DefaultZeroAddColumn(Intent::Column(Candidate::Inspect(actual))))
                    if actual == expected));
        }
    }
}

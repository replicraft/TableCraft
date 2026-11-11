//! Unsupported hosts expose inert unavailable facts only.
use crate::{
    Error, Response, controls::check, display::Presentation, document::Document,
    editing::FieldAdditionAvailability,
};

pub(in crate::editing) fn inspect(
    document: &mut Document,
    table: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    let mut output = Presentation::new(cancelled);
    Ok(Response::FieldAdditionAvailability {
        availability: FieldAdditionAvailability {
            document_revision: document.revision,
            table: output.text(table)?,
            number_available: false,
            short_text_available: false,
            typed_declarations: std::sync::Arc::from([]),
        },
    })
}

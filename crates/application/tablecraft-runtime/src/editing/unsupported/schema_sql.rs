//! Unsupported platforms cannot mint a native schema-review capability.
use crate::{Error, Response, document::Document, editing::EditError};
use tablecraft_sql::query::definition::Definition;

pub(in crate::editing) fn review(
    _document: &mut Document,
    _definition: &Definition,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

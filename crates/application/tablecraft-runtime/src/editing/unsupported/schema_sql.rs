//! Unsupported platforms cannot mint a native schema-review capability.
use crate::{Error, Response, document::Document, editing::EditError};
#[cfg(feature = "ace16-create")]
use tablecraft_sql::query::Query;
use tablecraft_sql::query::definition::Definition;

pub(in crate::editing) fn review(
    _document: &mut Document,
    _definition: &Definition,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

#[cfg(feature = "ace16-create")]
pub(in crate::editing) fn review_make_table(
    _document: &mut Document,
    _query: Query,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

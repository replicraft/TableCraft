//! Unsupported platforms explicitly refuse source qualification, without proof.
use crate::{Error, Response, document::Document, editing::EditError};

pub(in crate::editing) fn inspect(
    _document: &mut Document,
    _table: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

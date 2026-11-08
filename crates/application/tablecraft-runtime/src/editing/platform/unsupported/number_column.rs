//! Unsupported hosts cannot issue a persisted Number declaration capability.
use crate::{
    Error, Response,
    document::Document,
    editing::{AddedColumnReopen, EditError},
};
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<AddedColumnReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

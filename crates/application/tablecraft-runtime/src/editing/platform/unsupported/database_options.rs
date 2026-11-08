//! Unsupported hosts cannot issue native database-option mutation authority.
use crate::{
    Error, Response,
    document::Document,
    editing::{DatabaseOptionsReopen, EditError},
};
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<DatabaseOptionsReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

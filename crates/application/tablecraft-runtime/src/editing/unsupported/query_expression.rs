//! Unsupported hosts issue no existing-query mutation capability.
use crate::{
    Error, Response, SavedQueryTarget,
    document::Document,
    editing::{EditError, QueryExpressionReopen},
};
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<QueryExpressionReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _target: &SavedQueryTarget,
    _expected: &str,
    _replacement: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

//! Hosts without native filesystem editing cannot issue a table DROP capability.
use crate::{
    Error, Response, document::Document,
    editing::{EditError, TableDroppedReopen},
};
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<TableDroppedReopen>,
}

pub(in crate::editing) fn inspect(
    _document: &mut Document,
    _table: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

//! Unsupported hosts cannot issue a native catalog mutation capability.
use crate::{
    Error, Response,
    document::Document,
    editing::{EditError, TableRenameReopen},
};
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<TableRenameReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _new_name: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

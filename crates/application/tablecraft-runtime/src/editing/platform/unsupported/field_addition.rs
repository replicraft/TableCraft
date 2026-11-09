//! Explicit unsupported-platform refusal; no native capability can be issued.
pub(in crate::editing) mod availability;
use crate::{
    Error, Response,
    document::Document,
    editing::{AddedColumnReopen, EditError},
};
use std::sync::Arc;
use tablecraft_builders::table::FieldAdditionDeclaration;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<AddedColumnReopen>,
    pub(in crate::editing) declaration: FieldAdditionDeclaration,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _declaration: FieldAdditionDeclaration,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

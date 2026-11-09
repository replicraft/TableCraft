//! Unsupported hosts preserve read metadata but issue no DROP save authority.
use crate::{
    Error, Response,
    controls::check,
    document::Document,
    editing::{EditError, FieldDroppedReopen},
};
use std::sync::Arc;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<FieldDroppedReopen>,
}

pub(in crate::editing) fn source_eligible(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _native_number: usize,
    _budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    check(cancelled)?;
    Ok(false)
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _native_number: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    check(cancelled)?;
    Err(EditError::UnsupportedPlatform.into())
}

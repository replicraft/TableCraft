//! Unsupported hosts cannot issue native database-option mutation authority.
use crate::{
    Error, Response,
    document::Document,
    editing::{DatabaseOptionsReopen, EditError},
};
use std::sync::Arc;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) struct State {
    pub(in crate::editing) reopen: Arc<DatabaseOptionsReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

/// Stored facts remain readable; unsupported hosts issue no native review grant.
pub(in crate::editing) fn source_available(
    _document: &Document,
    _work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    crate::controls::check(cancelled)?;
    Ok(false)
}

//! Unsupported hosts cannot issue a field-rename save capability.
use crate::{
    Error, Response,
    document::Document,
    editing::{EditError, FieldRenamedReopen},
};
use std::sync::{Arc, atomic::AtomicBool};

pub(in crate::editing) struct State {
    pub(in crate::editing) valid: Arc<AtomicBool>,
    pub(in crate::editing) reopen: Arc<FieldRenamedReopen>,
}

pub(in crate::editing) fn review(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _native_number: usize,
    _new_name: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    crate::controls::check(cancelled)?;
    Err(EditError::UnsupportedPlatform.into())
}

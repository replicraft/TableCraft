//! Unsupported hosts retain nominal facts without constructing save authority.
use crate::{
    Error, QueryCreatedReopen, QueryCreationTarget, Response, SavedQueryTarget, document::Document,
    editing::EditError,
};
use std::sync::Arc;
use tablecraft_expressions::functions::dates::clock::Clock;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<QueryCreatedReopen>,
}

pub(in crate::editing) fn review_sql(
    _document: &mut Document,
    _target: &mut QueryCreationTarget,
    _sql: &mut String,
    _destination: &str,
    _clock: &dyn Clock,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

pub(in crate::editing) fn review_saved(
    _document: &mut Document,
    _target: &mut SavedQueryTarget,
    _destination: &str,
    _clock: &dyn Clock,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

impl Document {
    pub(crate) fn open_query_created(
        _witness: &QueryCreatedReopen,
        _revision: u64,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<(Self, Response), Error> {
        Err(EditError::UnsupportedPlatform.into())
    }
}

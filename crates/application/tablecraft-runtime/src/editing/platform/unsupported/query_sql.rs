//! Unsupported hosts expose readonly definitions but issue no full-SQL capability.
use crate::{
    Error, QuerySqlEligibility, Response, SavedQueryTarget,
    controls::check,
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
    _original: &str,
    _proposed: &str,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    Err(EditError::UnsupportedPlatform.into())
}

pub(in crate::editing) fn eligibility(
    _document: &Document,
    _target: &SavedQueryTarget,
    _sql: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<QuerySqlEligibility, Error> {
    check(cancelled)?;
    Ok(QuerySqlEligibility::Unavailable)
}

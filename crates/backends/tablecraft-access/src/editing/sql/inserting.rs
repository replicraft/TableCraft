//! Normal INSERT binds complete explicit values through the shared query service.
use crate::editing::{
    NativeEditError,
    journal::{self, NativeJournalInserter},
};
use tablecraft_ace::{NativeImage, RecoverableNativeInsertPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::{InsertError, QueryError};
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Failure before any journal or target mutation exists.
#[derive(Debug, Error)]
pub enum RecoverableInsertError {
    /// Not a closed local complete-row INSERT VALUES request.
    #[error(transparent)]
    Shape(#[from] QueryError),
    /// Actual native metadata inspection failed.
    #[error(transparent)]
    Editor(#[from] NativeEditError),
    /// Expression binding, exact carriers or native INSERT admission refused.
    #[error(transparent)]
    Plan(#[from] InsertError<journal::Error>),
}

/// Reject external sources, SELECT, parameters and unsupported shape before open.
pub fn local_insert_table<'a>(
    query: &'a Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a str, QueryError> {
    tablecraft_query::local_insert_table(query, false, budget, cancelled)
}

/// Bind explicit values once; the native finish independently qualifies BEFORE.
pub fn plan_recoverable_insert(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeInsertPlan, RecoverableInsertError> {
    let table = local_insert_table(query, budget, cancelled)?;
    let inserter = NativeJournalInserter::new(image, table, budget, cancelled)?;
    Ok(tablecraft_query::plan_insert(
        inserter, query, budget, cancelled,
    )?)
}

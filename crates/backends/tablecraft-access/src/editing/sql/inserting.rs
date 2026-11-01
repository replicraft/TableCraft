//! Normal INSERT retains complete values or explicit original field presence.
use crate::editing::{
    NativeEditError,
    journal::{self, NativeJournalInsertPlan, NativeJournalInserter},
};
use tablecraft_ace::NativeImage;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::InsertableTable;
use tablecraft_query::{InsertError, QueryError};
use tablecraft_sql::query::{Query, Statement};
use thiserror::Error;

/// Failure before any journal or target mutation exists.
#[derive(Debug, Error)]
pub enum RecoverableInsertError {
    /// Not a closed local INSERT VALUES request.
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

/// Bind supplied expressions once and retain omitted fields through native finish.
pub fn plan_recoverable_insert(
    image: NativeImage,
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeJournalInsertPlan, RecoverableInsertError> {
    let table = local_insert_table(query, budget, cancelled)?;
    let inserter = NativeJournalInserter::new(image, table, budget, cancelled)?;
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported.into());
    };
    // Choose original presence structurally from the written field list and
    // actual metadata, before expressions or native planning. No failed plan
    // is retried under another default/complete-row policy.
    let omitted = !insert.fields.is_empty() && insert.fields.len() < inserter.columns().len();
    if omitted {
        Ok(tablecraft_query::plan_insert_with_omissions(
            inserter, query, budget, cancelled,
        )?)
    } else {
        Ok(tablecraft_query::plan_insert(
            inserter, query, budget, cancelled,
        )?)
    }
}

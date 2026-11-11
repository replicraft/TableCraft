//! Explicit correlated predicate reads retain closed mutation function binding.

use super::{BoundDelete, bind_with};
use crate::{QueryError, Source};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::query::Query;

pub(crate) fn bind(
    query: &Query,
    source: &dyn Source,
    table: &str,
    columns: &[tablecraft_schema::Column],
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundDelete, QueryError> {
    if let tablecraft_sql::query::Statement::Delete(delete) = &query.statement
        && let Some(filter) = &delete.selection
    {
        crate::binding::mutation_reads::syntax::predicate(filter, budget, cancelled)?;
    }
    bind_with(
        query,
        source,
        allow_parameters,
        budget,
        cancelled,
        Some((table, columns)),
    )
}

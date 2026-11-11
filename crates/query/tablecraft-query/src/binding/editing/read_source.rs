//! Explicit predicate reads do not admit scalar subqueries in SET expressions.
use super::{BoundUpdate, bind_assignments};
use crate::{QueryError, Source, binding::mutation_reads::syntax};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_sql::query::{Query, Statement};

/// Borrowed original target facts captured together before predicate binding.
pub(crate) struct ReadSourceUpdateTarget<'a> {
    pub table: &'a str,
    pub columns: &'a [tablecraft_schema::Column],
    pub text_collations: &'a [Option<crate::source::TextCollationId>],
}

pub(crate) fn bind(
    query: &Query,
    source: &dyn Source,
    target: ReadSourceUpdateTarget<'_>,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundUpdate, QueryError> {
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    for assignment in &update.assignments {
        syntax::assignment(&assignment.value, budget, cancelled)?;
    }
    if let Some(filter) = &update.selection {
        syntax::predicate(filter, budget, cancelled)?;
    }
    bind_assignments(
        query,
        source,
        true,
        allow_parameters,
        budget,
        cancelled,
        Some(target),
    )
}

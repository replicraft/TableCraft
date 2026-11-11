//! Explicit correlated predicate reads retain closed mutation function binding.
mod syntax;

use super::{BoundDelete, bind_with};
use crate::{
    QueryError, Source,
    binding::Binder,
    plan::{Expr, ReadNode, ReadPlan, RelationNode},
};
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
        syntax::expression(filter, budget, cancelled, 1)?;
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

pub(super) fn admit(expression: &Expr, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.work()?;
    match expression {
        Expr::Parameter(_)
        | Expr::Literal(_)
        | Expr::NumericLiteral(_)
        | Expr::DateLiteral(..)
        | Expr::Field { .. } => Ok(()),
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::Convert(_, value)
        | Expr::DateCall(_, value) => admit(value, binder),
        Expr::Binary(_, left, right) => {
            admit(left, binder)?;
            admit(right, binder)
        }
        Expr::Between(value, first, last, _) | Expr::IIf(value, first, last) => {
            admit(value, binder)?;
            admit(first, binder)?;
            admit(last, binder)
        }
        Expr::InList(value, values, _) => {
            admit(value, binder)?;
            for value in values {
                admit(value, binder)?;
            }
            Ok(())
        }
        Expr::Nz(value, replacement) => {
            admit(value, binder)?;
            if let Some(value) = replacement {
                admit(value, binder)?;
            }
            Ok(())
        }
        // Binder::mutation closes callable admission before this structural walk.
        Expr::TextCall(_, values)
        | Expr::MathCall(_, values)
        | Expr::ConstructorCall(_, values) => {
            for value in values {
                admit(value, binder)?;
            }
            Ok(())
        }
        Expr::Exists(read) => admit_read(read, binder),
        Expr::InQuery(value, read, _) => {
            admit(value, binder)?;
            admit_read(read, binder)
        }
        _ => Err(QueryError::Unsupported),
    }
}

fn admit_read(read: &ReadPlan, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.work()?;
    let ReadNode::Select(select) = &read.node else {
        return Err(QueryError::Unsupported);
    };
    if read.distinct
        || read.top.is_some()
        || !read.sort.is_empty()
        || select.aggregate
        || !select.group.is_empty()
        || select.having.is_some()
        || !select.record_keys.is_empty()
        || select.parent.is_some()
        || !read.recordset_schemas.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    let RelationNode::Table {
        namespace,
        multivalues,
        members,
        recordsets,
        ..
    } = &select.source.node
    else {
        return Err(QueryError::Unsupported);
    };
    if *namespace != crate::external_sources::SourceNamespace::Root
        || !multivalues.is_empty()
        || !members.is_empty()
        || !recordsets.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    for value in &select.projection {
        admit(value, binder)?;
    }
    if let Some(value) = &select.filter {
        admit(value, binder)?;
    }
    Ok(())
}

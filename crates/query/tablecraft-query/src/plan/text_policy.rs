//! Bounded static expression provenance; no row evaluation or comparer activation.
use super::{Aggregate, Expr};
use crate::{QueryError, profile::MAX_BIND_DEPTH, source::TextPolicy};
use tablecraft_sql::select::BinaryOperator;

pub(crate) fn expression(
    expr: &Expr,
    tick: &mut dyn FnMut() -> Result<(), QueryError>,
    depth: usize,
) -> Result<TextPolicy, QueryError> {
    tick()?;
    if depth > MAX_BIND_DEPTH {
        return Err(QueryError::StructureLimit);
    }
    Ok(match expr {
        Expr::Field { text_policy, .. }
        | Expr::CollectionItem { text_policy, .. }
        | Expr::Output(_, text_policy) => *text_policy,
        Expr::Binary(BinaryOperator::Concatenate | BinaryOperator::Add, left, right) => {
            expression(left, tick, depth + 1)?.merge(expression(right, tick, depth + 1)?)
        }
        Expr::IIf(_, left, right) => {
            expression(left, tick, depth + 1)?.merge(expression(right, tick, depth + 1)?)
        }
        Expr::Nz(value, replacement) => {
            expression(value, tick, depth + 1)?.merge(match replacement {
                Some(expr) => expression(expr, tick, depth + 1)?,
                None => TextPolicy::Default,
            })
        }
        Expr::Aggregate(
            Aggregate::First | Aggregate::Last | Aggregate::Minimum | Aggregate::Maximum,
            Some(expr),
        ) => expression(expr, tick, depth + 1)?,
        Expr::Subquery(plan) => *plan.text_policies.first().ok_or(QueryError::Width)?,
        // Literals/parameters, fresh built-in/conversion results, numeric/Boolean
        // results have host-default provenance.
        // This provisional model does not infer a native function's field policy.
        _ => TextPolicy::Default,
    })
}

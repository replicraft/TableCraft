//! Bounded static expression provenance; no row evaluation or comparer activation.
use super::{Aggregate, ConditionalFunction, Expr};
use crate::{BindingDepth, QueryError, source::TextPolicy};
use tablecraft_sql::select::BinaryOperator;

pub(crate) fn expression(
    expr: &Expr,
    tick: &mut dyn FnMut() -> Result<(), QueryError>,
    depth: usize,
    binding_depth: BindingDepth,
) -> Result<TextPolicy, QueryError> {
    tick()?;
    if depth > binding_depth.get() {
        return Err(QueryError::StructureLimit);
    }
    Ok(match expr {
        Expr::Multivalue { text_policy, .. } => *text_policy,
        Expr::Field { text_policy, .. }
        | Expr::CollectionItem { text_policy, .. }
        | Expr::Output(_, text_policy) => *text_policy,
        Expr::Projected(reference) => reference.text_policy,
        Expr::CVar(value) => expression(value, tick, depth + 1, binding_depth)?,
        Expr::Binary(BinaryOperator::Concatenate | BinaryOperator::Add, left, right)
        | Expr::IIf(_, left, right) => {
            let left = expression(left, tick, depth + 1, binding_depth)?;
            let right = expression(right, tick, depth + 1, binding_depth)?;
            left.merge(right)
        }
        Expr::ConditionalCall(function, arguments) => {
            let mut policy = TextPolicy::Default;
            for (index, argument) in arguments.iter().enumerate() {
                tick()?;
                let is_value = match function {
                    ConditionalFunction::Choose => index > 0,
                    ConditionalFunction::Switch => index % 2 == 1,
                };
                if is_value {
                    policy = policy.merge(expression(argument, tick, depth + 1, binding_depth)?);
                }
            }
            policy
        }
        Expr::Nz(value, replacement) => {
            expression(value, tick, depth + 1, binding_depth)?.merge(match replacement {
                Some(expr) => expression(expr, tick, depth + 1, binding_depth)?,
                None => TextPolicy::Default,
            })
        }
        Expr::Aggregate(
            Aggregate::First | Aggregate::Last | Aggregate::Minimum | Aggregate::Maximum,
            Some(expr),
        ) => expression(expr, tick, depth + 1, binding_depth)?,
        Expr::Subquery(plan) => *plan.text_policies.first().ok_or(QueryError::Width)?,
        // Literals/parameters, fresh built-in/conversion results, numeric/Boolean
        // results have host-default provenance.
        // This provisional model does not infer a native function's field policy.
        _ => TextPolicy::Default,
    })
}

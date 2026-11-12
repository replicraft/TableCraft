//! Paid traversal admits closed scalar/aggregate syntax without source expansion.
use super::Error;
use super::scope::Scope;
use crate::editing::sql::make_table::shape::check;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{arrays::sql_arguments, functions::Builtin};
use tablecraft_sql::select::{Expression, FunctionArgument};

#[derive(Clone, Copy)]
pub(super) enum Clause {
    Projection,
    Group,
    Having,
    Order,
}

pub(super) fn qualify(
    expression: &Expression,
    scope: &Scope<'_>,
    clause: Clause,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let mut pending = Vec::new();
    push(&mut pending, expression, work, cancelled)?;
    while let Some(expression) = pending.pop() {
        check(work, cancelled)?;
        match expression {
            Expression::Field(reference) => {
                scope.field(
                    reference,
                    matches!(clause, Clause::Having | Clause::Order),
                    work,
                    cancelled,
                )?;
            }
            Expression::Integer(_)
            | Expression::Text(_)
            | Expression::Number(_)
            | Expression::DateLiteral(_)
            | Expression::GuidLiteral(_)
            | Expression::Null
            | Expression::Boolean(_) => {}
            Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
                push(&mut pending, expression, work, cancelled)?;
            }
            Expression::Binary { left, right, .. } => {
                push(&mut pending, right, work, cancelled)?;
                push(&mut pending, left, work, cancelled)?;
            }
            Expression::Like {
                expression,
                pattern,
                ..
            } => {
                push(&mut pending, pattern, work, cancelled)?;
                push(&mut pending, expression, work, cancelled)?;
            }
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                ..
            } => {
                push(&mut pending, second_bound, work, cancelled)?;
                push(&mut pending, first_bound, work, cancelled)?;
                push(&mut pending, expression, work, cancelled)?;
            }
            Expression::InList {
                expression, values, ..
            } => {
                for value in values.iter().rev() {
                    push(&mut pending, value, work, cancelled)?;
                }
                push(&mut pending, expression, work, cancelled)?;
            }
            Expression::Call {
                qualifier,
                name,
                arguments,
            } => {
                work.charge(Resource::Steps, name.as_str().len() as u64)
                    .map_err(InspectError::from)?;
                // Registry builtins use scalar services, never native Source resolution.
                // Closed Eval independently validates its parsed expression and runs
                // with NoBindings; dynamic fields/domains/UDFs cannot reopen this source.
                if !qualifier.is_empty()
                    || (Builtin::resolve(name.as_str()).is_none()
                        && !sql_arguments::is_constructor(qualifier, name)
                        && !(matches!(clause, Clause::Projection | Clause::Having | Clause::Order)
                            && aggregate(name.as_str())))
                {
                    return Err(Error::Shape);
                }
                for argument in arguments.iter().rev() {
                    if let FunctionArgument::Expression(argument) = argument {
                        push(&mut pending, argument, work, cancelled)?;
                    }
                    // Existing binding owns optional/missing/wildcard validity.
                    // None of these forms contains a hidden expression to traverse.
                }
            }
            // No computed-projection subquery/domain/member/identity expansion.
            // The complete query evaluator retains its existing WHERE behavior.
            _ => return Err(Error::Shape),
        }
    }
    Ok(())
}

// This is the native publisher's explicit aggregate admission, independent of
// future query builtins. The query binder still validates arity, nesting, GROUP
// membership, optional slots and static result declarations.
fn aggregate(name: &str) -> bool {
    [
        "COUNT", "SUM", "AVG", "MIN", "MAX", "FIRST", "LAST", "VAR", "VARP", "STDEV", "STDEVP",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn push<'a>(
    pending: &mut Vec<&'a Expression>,
    expression: &'a Expression,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(work, cancelled)?;
    if pending.len() == pending.capacity() {
        let additional = pending.capacity().max(1);
        work.charge_allocation(additional as u64, size_of::<&Expression>() as u64)
            .map_err(InspectError::from)?;
        work.charge(Resource::Steps, pending.len() as u64)
            .map_err(InspectError::from)?;
        pending
            .try_reserve_exact(additional)
            .map_err(|_| InspectError::Allocation)?;
    }
    pending.push(expression);
    Ok(())
}

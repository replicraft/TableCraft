//! Closed deterministic scalar calls shared by original-row UPDATE and DELETE.
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_expressions::functions::{
    Builtin,
    math::MathFunction,
    text::{
        TextFunction,
        sql_arguments::{self, Argument},
    },
};
use tablecraft_sql::select::{Expression, FunctionArgument, Name};

pub(super) fn bind_call(
    binder: &mut Binder<'_>,
    qualifier: &[Name],
    name: &Name,
    arguments: &[FunctionArgument],
    scopes: &[&[Slot]],
    depth: usize,
) -> Result<Expr, QueryError> {
    if !qualifier.is_empty() {
        return Err(QueryError::Unsupported);
    }

    match (Builtin::resolve(name.as_str()), arguments) {
        (Some(Builtin::Convert(conversion)), [FunctionArgument::Expression(value)]) => {
            Ok(Expr::Convert(
                conversion,
                Box::new(binder.expression(value, scopes, false, depth + 1)?),
            ))
        }
        (
            Some(Builtin::IIf),
            [
                FunctionArgument::Expression(condition),
                FunctionArgument::Expression(yes),
                FunctionArgument::Expression(no),
            ],
        ) => Ok(Expr::IIf(
            Box::new(binder.expression(condition, scopes, false, depth + 1)?),
            Box::new(binder.expression(yes, scopes, false, depth + 1)?),
            Box::new(binder.expression(no, scopes, false, depth + 1)?),
        )),
        (Some(Builtin::Nz), [FunctionArgument::Expression(value)]) => Ok(Expr::Nz(
            Box::new(binder.expression(value, scopes, false, depth + 1)?),
            None,
        )),
        (
            Some(Builtin::Nz),
            [
                FunctionArgument::Expression(value),
                FunctionArgument::Expression(replacement),
            ],
        ) => Ok(Expr::Nz(
            Box::new(binder.expression(value, scopes, false, depth + 1)?),
            Some(Box::new(binder.expression(
                replacement,
                scopes,
                false,
                depth + 1,
            )?)),
        )),
        (Some(Builtin::Text(function)), arguments) if text_allowed(function) => {
            let arguments =
                sql_arguments::normalize(function, arguments).ok_or(QueryError::Unsupported)?;
            binder.charge(arguments.len() * size_of::<Expr>())?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(arguments.len())
                .map_err(|_| QueryError::Allocation)?;
            for argument in arguments {
                values.push(match argument {
                    Argument::Expression(value) => {
                        binder.expression(value, scopes, false, depth + 1)?
                    }
                    Argument::Integer(value) => Expr::Literal(Expression::Integer(value)),
                    Argument::EmptyText => return Err(QueryError::Unsupported),
                });
            }
            Ok(Expr::TextCall(function, values))
        }
        (Some(Builtin::Math(function)), arguments)
            if math_allowed(function) && function.accepts_arity(arguments.len()) =>
        {
            binder.charge(arguments.len() * size_of::<Expr>())?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(arguments.len())
                .map_err(|_| QueryError::Allocation)?;
            for argument in arguments {
                let FunctionArgument::Expression(value) = argument else {
                    return Err(QueryError::Unsupported);
                };
                values.push(binder.expression(value, scopes, false, depth + 1)?);
            }
            Ok(Expr::MathCall(function, values))
        }
        (Some(Builtin::Date(function)), [FunctionArgument::Expression(value)]) => {
            Ok(Expr::DateCall(
                function,
                Box::new(binder.expression(value, scopes, false, depth + 1)?),
            ))
        }
        (Some(Builtin::DateConstructor(function)), arguments)
            if function.accepts_arity(arguments.len()) =>
        {
            binder.charge(arguments.len() * size_of::<Expr>())?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(arguments.len())
                .map_err(|_| QueryError::Allocation)?;
            for argument in arguments {
                let FunctionArgument::Expression(value) = argument else {
                    return Err(QueryError::Unsupported);
                };
                values.push(binder.expression(value, scopes, false, depth + 1)?);
            }
            Ok(Expr::ConstructorCall(function, values))
        }
        _ => Err(QueryError::Unsupported),
    }
}

/// Reject every accidental retrieval/runtime node, even when not selected at runtime.
pub(super) fn admitted(expression: &Expr) -> bool {
    match expression {
        Expr::Literal(_)
        | Expr::NumericLiteral(_)
        | Expr::DateLiteral(..)
        | Expr::Field { depth: 0, .. } => true,
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::Convert(_, value)
        | Expr::DateCall(_, value) => admitted(value),
        Expr::Binary(_, left, right) => admitted(left) && admitted(right),
        Expr::Between(value, first, last, _) | Expr::IIf(value, first, last) => {
            admitted(value) && admitted(first) && admitted(last)
        }
        Expr::InList(value, values, _) => admitted(value) && values.iter().all(admitted),
        Expr::Nz(value, replacement) => {
            admitted(value) && replacement.as_ref().is_none_or(|value| admitted(value))
        }
        Expr::TextCall(function, values) => text_allowed(*function) && values.iter().all(admitted),
        Expr::MathCall(function, values) => math_allowed(*function) && values.iter().all(admitted),
        Expr::ConstructorCall(function, values) => {
            function.accepts_arity(values.len()) && values.iter().all(admitted)
        }
        _ => false,
    }
}

fn text_allowed(function: TextFunction) -> bool {
    matches!(
        function,
        TextFunction::Len
            | TextFunction::Left
            | TextFunction::Right
            | TextFunction::Mid
            | TextFunction::LTrim
            | TextFunction::RTrim
            | TextFunction::Trim
    )
}

fn math_allowed(function: MathFunction) -> bool {
    matches!(
        function,
        MathFunction::Abs | MathFunction::Int | MathFunction::Fix | MathFunction::Round
    )
}

//! Closed deterministic scalar calls shared by original-row UPDATE and DELETE.
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_expressions::functions::{
    Builtin,
    dates::parsing::DateConversion,
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
    if !qualifier.is_empty() || !call_allowed(name.as_str(), arguments) {
        return Err(QueryError::Unsupported);
    }

    match (Builtin::resolve(name.as_str()), arguments) {
        (Some(Builtin::IsNull), [FunctionArgument::Expression(value)]) => Ok(Expr::IsNull(
            Box::new(binder.expression(value, scopes, false, depth + 1)?),
            false,
        )),
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
        (Some(Builtin::DateConvert(function)), [FunctionArgument::Expression(value)])
            if date_conversion_allowed(function) =>
        {
            Ok(Expr::DateConvert(
                function,
                Box::new(binder.expression(value, scopes, false, depth + 1)?),
            ))
        }
        (
            Some(function @ (Builtin::DateAdd | Builtin::DateDiff | Builtin::DatePart)),
            arguments,
        ) => super::date_intervals::bind_call(binder, function, arguments, scopes, false, depth),
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

/// Classify the same closed scalar call language before metadata or binding.
/// This observes only call identity and written argument slots; callers must
/// recursively validate children and bind/evaluate them with their own context.
pub(crate) fn call_allowed(name: &str, arguments: &[FunctionArgument]) -> bool {
    let expressions = || {
        arguments
            .iter()
            .all(|argument| matches!(argument, FunctionArgument::Expression(_)))
    };
    match Builtin::resolve(name) {
        Some(Builtin::IsNull | Builtin::Convert(_) | Builtin::Date(_)) => {
            arguments.len() == 1 && expressions()
        }
        Some(Builtin::IIf) => arguments.len() == 3 && expressions(),
        Some(Builtin::Nz) => matches!(arguments.len(), 1 | 2) && expressions(),
        Some(Builtin::Text(function)) => {
            text_allowed(function) && sql_arguments::normalize(function, arguments).is_some()
        }
        Some(Builtin::Math(function)) => {
            math_allowed(function) && function.accepts_arity(arguments.len()) && expressions()
        }
        Some(Builtin::DateConvert(function)) => {
            date_conversion_allowed(function) && arguments.len() == 1 && expressions()
        }
        Some(Builtin::DateConstructor(function)) => {
            function.accepts_arity(arguments.len()) && expressions()
        }
        Some(Builtin::DateAdd) => arguments.len() == 3 && expressions(),
        Some(function @ (Builtin::DateDiff | Builtin::DatePart)) => {
            let required = if matches!(function, Builtin::DateDiff) {
                3
            } else {
                2
            };
            (required..=required + 2).contains(&arguments.len())
                && arguments[..required]
                    .iter()
                    .all(|argument| matches!(argument, FunctionArgument::Expression(_)))
                && arguments[required..].iter().all(|argument| {
                    matches!(
                        argument,
                        FunctionArgument::Expression(_) | FunctionArgument::Missing
                    )
                })
        }
        _ => false,
    }
}

/// Reject every accidental retrieval/runtime node, even when not selected at runtime.
pub(super) fn admitted(expression: &Expr) -> bool {
    admitted_with(expression, false)
}

/// Criteria share scalar admission while retaining explicit wildcard context.
pub(super) fn predicate_admitted(expression: &Expr) -> bool {
    admitted_with(expression, true)
}

fn admitted_with(expression: &Expr, like: bool) -> bool {
    match expression {
        Expr::Parameter(_)
        | Expr::Literal(_)
        | Expr::NumericLiteral(_)
        | Expr::DateLiteral(..)
        | Expr::Field { depth: 0, .. } => true,
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::Convert(_, value)
        | Expr::DateCall(_, value) => admitted_with(value, like),
        Expr::Like(left, right, _) if like => {
            admitted_with(left, like) && admitted_with(right, like)
        }
        Expr::Binary(_, left, right) => admitted_with(left, like) && admitted_with(right, like),
        Expr::Between(value, first, last, _)
        | Expr::IIf(value, first, last)
        | Expr::DateAdd(value, first, last) => {
            admitted_with(value, like) && admitted_with(first, like) && admitted_with(last, like)
        }
        Expr::DatePart(interval, date, options) => {
            admitted_with(interval, like)
                && admitted_with(date, like)
                && options
                    .iter()
                    .flatten()
                    .all(|value| admitted_with(value, like))
        }
        Expr::DateDiff(interval, first, second, options) => {
            admitted_with(interval, like)
                && admitted_with(first, like)
                && admitted_with(second, like)
                && options
                    .iter()
                    .flatten()
                    .all(|value| admitted_with(value, like))
        }
        Expr::InList(value, values, _) => {
            admitted_with(value, like) && values.iter().all(|value| admitted_with(value, like))
        }
        Expr::Nz(value, replacement) => {
            admitted_with(value, like)
                && replacement
                    .as_ref()
                    .is_none_or(|value| admitted_with(value, like))
        }
        Expr::DateConvert(function, value) => {
            date_conversion_allowed(*function) && admitted_with(value, like)
        }
        Expr::TextCall(function, values) => {
            text_allowed(*function) && values.iter().all(|value| admitted_with(value, like))
        }
        Expr::MathCall(function, values) => {
            math_allowed(*function) && values.iter().all(|value| admitted_with(value, like))
        }
        Expr::ConstructorCall(function, values) => {
            function.accepts_arity(values.len())
                && values.iter().all(|value| admitted_with(value, like))
        }
        _ => false,
    }
}

fn text_allowed(function: TextFunction) -> bool {
    matches!(
        function,
        TextFunction::CStr
            | TextFunction::Len
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
        MathFunction::Abs
            | MathFunction::Sgn
            | MathFunction::Int
            | MathFunction::Fix
            | MathFunction::Sqr
            | MathFunction::Exp
            | MathFunction::Log
            | MathFunction::Sin
            | MathFunction::Cos
            | MathFunction::Tan
            | MathFunction::Atn
            | MathFunction::Round
            | MathFunction::Rgb
            | MathFunction::QbColor
    )
}

pub(super) fn date_conversion_allowed(function: DateConversion) -> bool {
    matches!(
        function,
        DateConversion::CDate | DateConversion::DateValue | DateConversion::TimeValue
    )
}

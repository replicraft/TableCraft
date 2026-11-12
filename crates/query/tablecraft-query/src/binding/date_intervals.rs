//! Shared lowering for explicit Gregorian interval calls and their omitted controls.
use super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
use tablecraft_expressions::functions::Builtin;
use tablecraft_sql::select::FunctionArgument as A;

pub(super) fn bind_call(
    binder: &mut Binder<'_>,
    function: Builtin,
    arguments: &[A],
    scopes: &[&[Slot]],
    aggregates: bool,
    depth: usize,
) -> Result<Expr, QueryError> {
    match (function, arguments) {
        (
            Builtin::DateAdd,
            [
                A::Expression(interval),
                A::Expression(number),
                A::Expression(date),
            ],
        ) => Ok(Expr::DateAdd(
            Box::new(binder.expression(interval, scopes, aggregates, depth + 1)?),
            Box::new(binder.expression(number, scopes, aggregates, depth + 1)?),
            Box::new(binder.expression(date, scopes, aggregates, depth + 1)?),
        )),
        (
            Builtin::DateDiff,
            [
                A::Expression(interval),
                A::Expression(first),
                A::Expression(second),
                options @ ..,
            ],
        ) if options.len() <= 2
            && options
                .iter()
                .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
        {
            let interval = Box::new(binder.expression(interval, scopes, aggregates, depth + 1)?);
            let first = Box::new(binder.expression(first, scopes, aggregates, depth + 1)?);
            let second = Box::new(binder.expression(second, scopes, aggregates, depth + 1)?);
            let mut settings = [None, None];

            for (argument, setting) in options.iter().zip(&mut settings) {
                if let A::Expression(expr) = argument {
                    *setting = Some(Box::new(binder.expression(
                        expr,
                        scopes,
                        aggregates,
                        depth + 1,
                    )?));
                }
            }

            Ok(Expr::DateDiff(interval, first, second, settings))
        }
        (Builtin::DatePart, [A::Expression(interval), A::Expression(date), options @ ..])
            if options.len() <= 2
                && options
                    .iter()
                    .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
        {
            let interval = Box::new(binder.expression(interval, scopes, aggregates, depth + 1)?);
            let date = Box::new(binder.expression(date, scopes, aggregates, depth + 1)?);
            let mut settings = [None, None];

            for (argument, setting) in options.iter().zip(&mut settings) {
                if let A::Expression(expr) = argument {
                    *setting = Some(Box::new(binder.expression(
                        expr,
                        scopes,
                        aggregates,
                        depth + 1,
                    )?));
                }
            }

            Ok(Expr::DatePart(interval, date, settings))
        }
        _ => Err(QueryError::Unsupported),
    }
}

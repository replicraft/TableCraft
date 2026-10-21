//! Bind only closed built-ins and supported unqualified aggregate forms.
use super::Binder;
use crate::{
    QueryError,
    plan::{Aggregate, DomainFunction, Expr, Slot, Statistic},
};
use tablecraft_expressions::functions::{
    Builtin,
    financial::sql_arguments::{self as financial_arguments, Argument as FinancialArgument},
};
use tablecraft_sql::select::{FunctionArgument as A, Name};

impl Binder<'_> {
    pub(super) fn call(
        &mut self,
        qualifier: &[Name],
        name: &Name,
        arguments: &[A],
        scopes: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Expr, QueryError> {
        if self.retrieval && qualifier.is_empty() {
            let builtin = Builtin::resolve(name.as_str());
            if let Some(Builtin::Text(function)) = builtin {
                use tablecraft_expressions::functions::text::sql_arguments::{self, Argument};
                let arguments =
                    sql_arguments::normalize(function, arguments).ok_or(QueryError::Unsupported)?;
                self.charge(arguments.len() * size_of::<Expr>())?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(arguments.len())
                    .map_err(|_| QueryError::Allocation)?;
                for argument in arguments {
                    values.push(match argument {
                        Argument::Expression(expression) => {
                            self.expression(expression, scopes, aggregates, depth + 1)?
                        }
                        Argument::Integer(default) => {
                            Expr::Literal(tablecraft_sql::select::Expression::Integer(default))
                        }
                    });
                }
                return Ok(Expr::TextCall(function, values));
            }
            match (builtin, arguments) {
                (Some(Builtin::Clock(function)), []) => return Ok(Expr::Clock(function)),
                (Some(Builtin::IsNull), [A::Expression(value)]) => {
                    return Ok(Expr::IsNull(
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                        false,
                    ));
                }
                (
                    Some(Builtin::IIf),
                    [
                        A::Expression(condition),
                        A::Expression(truepart),
                        A::Expression(falsepart),
                    ],
                ) => {
                    return Ok(Expr::IIf(
                        Box::new(self.expression(condition, scopes, aggregates, depth + 1)?),
                        Box::new(self.expression(truepart, scopes, aggregates, depth + 1)?),
                        Box::new(self.expression(falsepart, scopes, aggregates, depth + 1)?),
                    ));
                }
                (Some(Builtin::Nz), [A::Expression(value)]) => {
                    return Ok(Expr::Nz(
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                        None,
                    ));
                }
                (Some(Builtin::Nz), [A::Expression(value), A::Expression(replacement)]) => {
                    return Ok(Expr::Nz(
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                        Some(Box::new(self.expression(
                            replacement,
                            scopes,
                            aggregates,
                            depth + 1,
                        )?)),
                    ));
                }
                (
                    Some(Builtin::DateAdd),
                    [
                        A::Expression(interval),
                        A::Expression(number),
                        A::Expression(date),
                    ],
                ) => {
                    return Ok(Expr::DateAdd(
                        Box::new(self.expression(interval, scopes, aggregates, depth + 1)?),
                        Box::new(self.expression(number, scopes, aggregates, depth + 1)?),
                        Box::new(self.expression(date, scopes, aggregates, depth + 1)?),
                    ));
                }
                (
                    Some(Builtin::DateDiff),
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
                    let interval =
                        Box::new(self.expression(interval, scopes, aggregates, depth + 1)?);
                    let first = Box::new(self.expression(first, scopes, aggregates, depth + 1)?);
                    let second =
                        Box::new(self.expression(second, scopes, aggregates, depth + 1)?);
                    let mut settings = [None, None];
                    for (argument, setting) in options.iter().zip(&mut settings) {
                        if let A::Expression(expr) = argument {
                            *setting = Some(Box::new(self.expression(
                                expr,
                                scopes,
                                aggregates,
                                depth + 1,
                            )?));
                        }
                    }
                    return Ok(Expr::DateDiff(interval, first, second, settings));
                }
                (
                    Some(Builtin::DatePart),
                    [A::Expression(interval), A::Expression(date), options @ ..],
                ) if options.len() <= 2
                    && options
                        .iter()
                        .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
                {
                    let interval =
                        Box::new(self.expression(interval, scopes, aggregates, depth + 1)?);
                    let date = Box::new(self.expression(date, scopes, aggregates, depth + 1)?);
                    let mut settings = [None, None];
                    for (argument, setting) in options.iter().zip(&mut settings) {
                        if let A::Expression(expr) = argument {
                            *setting = Some(Box::new(self.expression(
                                expr,
                                scopes,
                                aggregates,
                                depth + 1,
                            )?));
                        }
                    }
                    return Ok(Expr::DatePart(interval, date, settings));
                }
                (Some(Builtin::Weekday), [A::Expression(value), options @ ..])
                    if options.len() <= 1
                        && options
                            .iter()
                            .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
                {
                    let value = Box::new(self.expression(value, scopes, aggregates, depth + 1)?);
                    let first_day = match options.first() {
                        Some(A::Expression(expr)) => Some(Box::new(self.expression(
                            expr,
                            scopes,
                            aggregates,
                            depth + 1,
                        )?)),
                        _ => None,
                    };
                    return Ok(Expr::Weekday(value, first_day));
                }
                (Some(Builtin::Convert(conversion)), [A::Expression(value)]) => {
                    return Ok(Expr::Convert(
                        conversion,
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                    ));
                }
                (Some(Builtin::DateConvert(function)), [A::Expression(value)]) => {
                    return Ok(Expr::DateConvert(
                        function,
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                    ));
                }
                (Some(Builtin::Inspect(function)), [A::Expression(value)]) => {
                    return Ok(Expr::Inspect(
                        function,
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                    ));
                }
                (Some(Builtin::Date(function)), [A::Expression(value)]) => {
                    return Ok(Expr::DateCall(
                        function,
                        Box::new(self.expression(value, scopes, aggregates, depth + 1)?),
                    ));
                }
                (Some(Builtin::Conditional(function)), arguments)
                    if function.accepts_arity(arguments.len())
                        && arguments.iter().all(|arg| matches!(arg, A::Expression(_))) =>
                {
                    self.charge(arguments.len() * size_of::<Expr>())?;
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(arguments.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for argument in arguments {
                        let A::Expression(expr) = argument else {
                            return Err(QueryError::Unsupported);
                        };
                        values.push(self.expression(expr, scopes, aggregates, depth + 1)?);
                    }
                    return Ok(Expr::ConditionalCall(function, values));
                }
                (Some(Builtin::Financial(function)), arguments) => {
                    let arguments = financial_arguments::normalize(function, arguments)
                        .ok_or(QueryError::Unsupported)?;
                    self.charge(arguments.len() * size_of::<Expr>())?;
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(arguments.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for argument in arguments {
                        values.push(match argument {
                            FinancialArgument::Expression(expr) => {
                                self.expression(expr, scopes, aggregates, depth + 1)?
                            }
                            FinancialArgument::Zero => {
                                Expr::Literal(tablecraft_sql::select::Expression::Integer(0))
                            }
                        });
                    }
                    return Ok(Expr::FinancialCall(function, values));
                }
                (Some(Builtin::Math(function)), arguments)
                    if function.accepts_arity(arguments.len())
                        && arguments.iter().all(|arg| matches!(arg, A::Expression(_))) =>
                {
                    self.charge(arguments.len() * size_of::<Expr>())?;
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(arguments.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for argument in arguments {
                        let A::Expression(expr) = argument else {
                            return Err(QueryError::Unsupported);
                        };
                        values.push(self.expression(expr, scopes, aggregates, depth + 1)?);
                    }
                    return Ok(Expr::MathCall(function, values));
                }
                (Some(Builtin::DateConstructor(function)), arguments)
                    if function.accepts_arity(arguments.len())
                        && arguments.iter().all(|arg| matches!(arg, A::Expression(_))) =>
                {
                    self.charge(arguments.len() * size_of::<Expr>())?;
                    let mut values = Vec::new();
                    values
                        .try_reserve_exact(arguments.len())
                        .map_err(|_| QueryError::Allocation)?;
                    for argument in arguments {
                        let A::Expression(expr) = argument else {
                            return Err(QueryError::Unsupported);
                        };
                        values.push(self.expression(expr, scopes, aggregates, depth + 1)?);
                    }
                    return Ok(Expr::ConstructorCall(function, values));
                }
                (Some(_), _) => return Err(QueryError::Unsupported),
                (None, _) => {}
            }
        }
        if self.retrieval
            && qualifier.is_empty()
            && let Some(kind) = DomainFunction::resolve(name.as_str())
        {
            if !(2..=3).contains(&arguments.len()) {
                return Err(QueryError::Unsupported);
            }
            // An omitted final optional slot is omission, never stored NULL.
            let arguments = match arguments {
                [A::Expression(_), A::Expression(_), A::Missing] => &arguments[..2],
                _ => arguments,
            };
            self.charge(arguments.len() * size_of::<Expr>())?;
            let mut bound = Vec::new();
            bound
                .try_reserve_exact(arguments.len())
                .map_err(|_| QueryError::Allocation)?;
            for argument in arguments {
                let A::Expression(expr) = argument else {
                    return Err(QueryError::Unsupported);
                };
                bound.push(self.expression(expr, scopes, aggregates, depth + 1)?);
            }
            return Ok(Expr::DomainCall(kind, bound));
        }
        if !aggregates || !qualifier.is_empty() || arguments.len() != 1 {
            return Err(QueryError::Unsupported);
        }
        let aggregate = aggregate(name.as_str()).ok_or(QueryError::Unsupported)?;
        let argument = match &arguments[0] {
            A::Wildcard if matches!(aggregate, Aggregate::Count) => None,
            A::Expression(e) => Some(Box::new(self.expression(e, scopes, false, depth + 1)?)),
            _ => return Err(QueryError::Unsupported),
        };
        Ok(Expr::Aggregate(aggregate, argument))
    }
}

fn aggregate(name: &str) -> Option<Aggregate> {
    [
        ("COUNT", Aggregate::Count),
        ("SUM", Aggregate::Sum),
        ("AVG", Aggregate::Average),
        ("MIN", Aggregate::Minimum),
        ("MAX", Aggregate::Maximum),
        ("FIRST", Aggregate::First),
        ("LAST", Aggregate::Last),
        ("VAR", Aggregate::Statistic(Statistic::SampleVariance)),
        ("VARP", Aggregate::Statistic(Statistic::PopulationVariance)),
        ("STDEV", Aggregate::Statistic(Statistic::SampleDeviation)),
        (
            "STDEVP",
            Aggregate::Statistic(Statistic::PopulationDeviation),
        ),
    ]
    .into_iter()
    .find_map(|(spelling, kind)| name.eq_ignore_ascii_case(spelling).then_some(kind))
}

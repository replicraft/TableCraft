//! Only known unqualified scalar calls reach value evaluation.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        self, Builtin,
        financial::{
            FinancialFunction,
            sql_arguments::{self as financial_arguments, Argument as FinancialArgument},
        },
        text::TextFunction,
    },
};
use tablecraft_core::values::{Double, Value};
use tablecraft_sql::select::{FunctionArgument as A, Name};

impl Evaluator<'_> {
    pub(super) fn call(
        &mut self,
        qualifier: &[Name],
        name: &Name,
        arguments: &[A],
        depth: usize,
    ) -> Result<Value, EvalError> {
        if !qualifier.is_empty() {
            return Err(EvalError::UnsupportedExpression);
        }
        let builtin = Builtin::resolve(name.as_str());
        if let Some(Builtin::Text(function)) = builtin {
            use crate::functions::text::sql_arguments::{self, Argument};
            let arguments = sql_arguments::normalize(function, arguments)
                .ok_or(EvalError::UnsupportedExpression)?;
            let count = arguments.len();
            let mut values: [Value; TextFunction::MAX_ARGUMENTS] =
                std::array::from_fn(|_| Value::Null);
            for (argument, value) in arguments.zip(&mut values) {
                *value = match argument {
                    Argument::Expression(expression) => self.node(expression, depth + 1)?,
                    Argument::Integer(default) => Value::Int64(default),
                };
            }
            return self.apply_text_function(function, &values[..count]);
        }
        match (builtin, arguments) {
            (Some(Builtin::ValueInspect(function)), [A::Expression(expression)]) => {
                let input = self.inspection_operand(expression, depth + 1)?;
                self.apply_value_inspection(function, input.inspection())
            }
            (Some(Builtin::ArrayBound(function)), arguments) => {
                self.array_bound(function, arguments, depth)
            }
            (Some(Builtin::CashFlow(function)), arguments) => {
                self.cashflows(function, arguments, depth)
            }
            (Some(Builtin::Clock(function)), []) => self.apply_clock(function),
            (Some(Builtin::IsNull), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                Ok(Value::Boolean(functions::is_null(&value)))
            }
            (
                Some(Builtin::IIf),
                [
                    A::Expression(condition),
                    A::Expression(truepart),
                    A::Expression(falsepart),
                ],
            ) => {
                let condition = self.node(condition, depth + 1)?;
                let truepart = self.node(truepart, depth + 1)?;
                let falsepart = self.node(falsepart, depth + 1)?;
                functions::iif(&condition, truepart, falsepart)
            }
            (Some(Builtin::Nz), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                Ok(functions::nz_query(value, None))
            }
            (Some(Builtin::Nz), [A::Expression(value), A::Expression(replacement)]) => {
                let value = self.node(value, depth + 1)?;
                let replacement = self.node(replacement, depth + 1)?;
                Ok(functions::nz_query(value, Some(replacement)))
            }
            (
                Some(Builtin::DateAdd),
                [
                    A::Expression(interval),
                    A::Expression(number),
                    A::Expression(date),
                ],
            ) => {
                let interval = self.node(interval, depth + 1)?;
                let number = self.node(number, depth + 1)?;
                let date = self.node(date, depth + 1)?;
                functions::dates::date_add(&interval, &number, &date)
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
                let interval = self.node(interval, depth + 1)?;
                let first = self.node(first, depth + 1)?;
                let second = self.node(second, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.node(expr, depth + 1)?);
                    }
                }
                functions::dates::date_diff_with_context(
                    &interval,
                    &first,
                    &second,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                    crate::functions::dates::WeekContext {
                        first_day: self.first_day_of_week,
                        first_week: self.first_week_of_year,
                    },
                )
            }
            (
                Some(Builtin::DatePart),
                [A::Expression(interval), A::Expression(date), options @ ..],
            ) if options.len() <= 2
                && options
                    .iter()
                    .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
            {
                let interval = self.node(interval, depth + 1)?;
                let date = self.node(date, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.node(expr, depth + 1)?);
                    }
                }
                functions::dates::date_part_with_context(
                    &interval,
                    &date,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                    crate::functions::dates::WeekContext {
                        first_day: self.first_day_of_week,
                        first_week: self.first_week_of_year,
                    },
                )
            }
            (Some(Builtin::Weekday), [A::Expression(value), options @ ..])
                if options.len() <= 1
                    && options
                        .iter()
                        .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
            {
                let value = self.node(value, depth + 1)?;
                let first_day = match options.first() {
                    Some(A::Expression(expr)) => Some(self.node(expr, depth + 1)?),
                    _ => None,
                };
                self.apply_weekday(&value, first_day.as_ref())
            }
            (Some(Builtin::Convert(conversion)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                self.apply_conversion(conversion, &value)
            }
            (Some(Builtin::DateConvert(function)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                self.apply_date_conversion(function, &value)
            }
            (Some(Builtin::Inspect(function)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                self.apply_inspection(function, &value)
            }
            (Some(Builtin::Date(function)), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                function.apply(&value)
            }
            (Some(Builtin::Conditional(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut selection = function.start(arguments.len())?;
                for argument in arguments {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    selection.push(self.node(expr, depth + 1)?)?;
                }
                selection.finish()
            }
            (Some(Builtin::Financial(function)), arguments) => {
                let arguments = financial_arguments::normalize(function, arguments)
                    .ok_or(EvalError::UnsupportedExpression)?;
                let count = arguments.len();
                let mut values: [Value; FinancialFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.zip(&mut values) {
                    *value = match argument {
                        FinancialArgument::Expression(expr) => self.node(expr, depth + 1)?,
                        FinancialArgument::Integer(default) => Value::Int64(default),
                        FinancialArgument::DefaultGuess => {
                            Value::Double(Double::from_float(FinancialFunction::DEFAULT_GUESS))
                        }
                    };
                }
                self.apply_financial(function, &values[..count])
            }
            (Some(Builtin::Math(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut values: [Value; crate::functions::math::MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    *value = self.node(expr, depth + 1)?;
                }
                function.apply(&values[..arguments.len()])
            }
            (Some(Builtin::DateConstructor(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut values: [Value; crate::functions::dates::DateConstructor::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    *value = self.node(expr, depth + 1)?;
                }
                function.apply_with_window(&values[..arguments.len()], self.year_window)
            }
            _ => Err(EvalError::UnsupportedExpression),
        }
    }
}

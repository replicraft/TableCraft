//! Only known unqualified scalar calls reach value evaluation.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        Builtin,
        conditional::expression as selection,
        conversions::errors::into_scalar,
        dates::DateConstructor,
        financial::{
            FinancialFunction,
            sql_arguments::{self as financial_arguments, Argument as FinancialArgument},
        },
        math::MathFunction,
        text::{
            TextFunction,
            sql_arguments::{self, Argument},
        },
    },
};
use tablecraft_core::values::{Double, ExpressionValue, Value};
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
            if function == TextFunction::CStr
                && let [A::Expression(expression)] = arguments
            {
                let value = self.expression_operand(expression, depth + 1)?;
                return self.apply_expression_cstr(&value);
            }
            if function == TextFunction::Str
                && let [A::Expression(expression)] = arguments
            {
                let value = self.expression_operand(expression, depth + 1)?;
                return self.apply_expression_str(&value);
            }
            if matches!(function, TextFunction::Hex | TextFunction::Oct)
                && let [A::Expression(expression)] = arguments
            {
                let value = self.expression_operand(expression, depth + 1)?;
                return self.apply_expression_radix(function, &value);
            }
            if function == TextFunction::InStrB {
                return self.byte_search_call(arguments, depth);
            }
            if function.produces_bytes() {
                let value = self.byte_call(function, arguments, depth)?;
                return self.publish_byte_operand(value);
            }
            if matches!(function, TextFunction::LenB | TextFunction::AscB)
                && let [A::Expression(expression)] = arguments
            {
                let value = self.byte_operand(expression, depth + 1)?;
                return self.observe_byte_operand(function, value);
            }
            let arguments = sql_arguments::normalize(function, arguments)
                .ok_or(EvalError::UnsupportedExpression)?;
            let count = arguments.len();
            if matches!(
                function,
                TextFunction::Left
                    | TextFunction::Right
                    | TextFunction::Mid
                    | TextFunction::InStr
                    | TextFunction::InStrRev
                    | TextFunction::Replace
                    | TextFunction::StrComp
            ) {
                let mut values: [ExpressionValue; TextFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.zip(&mut values) {
                    *value = match argument {
                        Argument::Expression(expression) => {
                            self.expression_operand(expression, depth + 1)?
                        }
                        Argument::Integer(default) => ExpressionValue::Value(Value::Int64(default)),
                        Argument::EmptyText => ExpressionValue::Value(Value::Text(String::new())),
                    };
                }
                return match function {
                    TextFunction::InStr | TextFunction::InStrRev => {
                        self.apply_expression_search(function, &values[..count])
                    }
                    TextFunction::Replace => self.apply_expression_replacement(&values[..count]),
                    TextFunction::StrComp => self.apply_expression_comparison(&values[..count]),
                    _ => self.apply_expression_slice(function, &values[..count]),
                };
            }
            let mut values: [Value; TextFunction::MAX_ARGUMENTS] =
                std::array::from_fn(|_| Value::Null);
            for (argument, value) in arguments.zip(&mut values) {
                *value = match argument {
                    Argument::Expression(expression) => self.node(expression, depth + 1)?,
                    Argument::Integer(default) => Value::Int64(default),
                    Argument::EmptyText => Value::Text(String::new()),
                };
            }
            return self.apply_text_function(function, &values[..count]);
        }
        match (builtin, arguments) {
            (Some(Builtin::CurrentUser), []) => self.apply_current_user(),
            (Some(Builtin::ErrorMessage | Builtin::ErrorMessageString), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                self.apply_expression_error_message(&value)
            }
            (Some(Builtin::Eval), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                into_scalar(self.dynamic_expression(&value, depth)?)
            }
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
            (Some(Builtin::GuidFromString), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                self.apply_guid_from_string(&value)
            }
            (Some(Builtin::StringFromGuid), [A::Expression(value)]) => {
                let value = self.node(value, depth + 1)?;
                self.apply_string_from_guid(&value)
            }
            (Some(Builtin::CVar), [A::Expression(value)]) => {
                into_scalar(self.expression_operand(value, depth + 1)?)
            }
            (Some(Builtin::CvErr), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                let value = self.apply_cverr(value)?;
                into_scalar(value)
            }
            (Some(Builtin::IsNull), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                Ok(Value::Boolean(matches!(
                    value,
                    ExpressionValue::Value(Value::Null)
                )))
            }
            (
                Some(Builtin::IIf),
                [
                    A::Expression(condition),
                    A::Expression(truepart),
                    A::Expression(falsepart),
                ],
            ) => {
                let condition = self.expression_operand(condition, depth + 1)?;
                let truepart = self.expression_operand(truepart, depth + 1)?;
                let falsepart = self.expression_operand(falsepart, depth + 1)?;
                into_scalar(self.apply_iif(&condition, truepart, falsepart)?)
            }
            (Some(Builtin::Nz), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                into_scalar(selection::nz(value, None))
            }
            (Some(Builtin::Nz), [A::Expression(value), A::Expression(replacement)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                let replacement = self.expression_operand(replacement, depth + 1)?;
                into_scalar(selection::nz(value, Some(replacement)))
            }
            (
                Some(Builtin::DateAdd),
                [
                    A::Expression(interval),
                    A::Expression(number),
                    A::Expression(date),
                ],
            ) => {
                let interval = self.expression_operand(interval, depth + 1)?;
                let number = self.expression_operand(number, depth + 1)?;
                let date = self.expression_operand(date, depth + 1)?;
                self.apply_expression_date_add(&interval, &number, &date)
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
                let interval = self.expression_operand(interval, depth + 1)?;
                let first = self.expression_operand(first, depth + 1)?;
                let second = self.expression_operand(second, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.expression_operand(expr, depth + 1)?);
                    }
                }
                self.apply_expression_date_diff(
                    &interval,
                    &first,
                    &second,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
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
                let interval = self.expression_operand(interval, depth + 1)?;
                let date = self.expression_operand(date, depth + 1)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let A::Expression(expr) = argument {
                        *setting = Some(self.expression_operand(expr, depth + 1)?);
                    }
                }
                self.apply_expression_date_part(
                    &interval,
                    &date,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )
            }
            (Some(Builtin::Weekday), [A::Expression(value), options @ ..])
                if options.len() <= 1
                    && options
                        .iter()
                        .all(|arg| matches!(arg, A::Expression(_) | A::Missing)) =>
            {
                let value = self.expression_operand(value, depth + 1)?;
                let first_day = match options.first() {
                    Some(A::Expression(expr)) => Some(self.expression_operand(expr, depth + 1)?),
                    _ => None,
                };
                self.apply_expression_weekday(&value, first_day.as_ref())
            }
            (Some(Builtin::Convert(conversion)), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                self.apply_expression_conversion(conversion, &value)
            }
            (Some(Builtin::DateConvert(function)), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                self.apply_expression_date_conversion(function, &value)
            }
            (Some(Builtin::Inspect(function)), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                self.apply_expression_inspection(function, &value)
            }
            (Some(Builtin::Date(function)), [A::Expression(value)]) => {
                let value = self.expression_operand(value, depth + 1)?;
                self.apply_expression_date_function(function, &value)
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
                    let value = self.expression_operand(expr, depth + 1)?;
                    self.push_selection_argument(&mut selection, value)?;
                }
                selection.finish()
            }
            (Some(Builtin::Financial(function)), arguments) => {
                let arguments = financial_arguments::normalize(function, arguments)
                    .ok_or(EvalError::UnsupportedExpression)?;
                let count = arguments.len();
                let mut values: [ExpressionValue; FinancialFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.zip(&mut values) {
                    *value = match argument {
                        FinancialArgument::Expression(expr) => {
                            self.expression_operand(expr, depth + 1)?
                        }
                        FinancialArgument::Integer(default) => {
                            ExpressionValue::Value(Value::Int64(default))
                        }
                        FinancialArgument::DefaultGuess => ExpressionValue::Value(Value::Double(
                            Double::from_float(FinancialFunction::DEFAULT_GUESS),
                        )),
                    };
                }
                self.apply_expression_financial(function, &values[..count])
            }
            (Some(Builtin::Math(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut values: [ExpressionValue; MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.iter().zip(&mut values) {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    *value = self.expression_operand(expr, depth + 1)?;
                }
                self.apply_expression_math(function, &values[..arguments.len()])
            }
            (Some(Builtin::DateConstructor(function)), arguments)
                if function.accepts_arity(arguments.len())
                    && arguments
                        .iter()
                        .all(|argument| matches!(argument, A::Expression(_))) =>
            {
                let mut values: [ExpressionValue; DateConstructor::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.iter().zip(&mut values) {
                    let A::Expression(expr) = argument else {
                        return Err(EvalError::UnsupportedExpression);
                    };
                    *value = self.expression_operand(expr, depth + 1)?;
                }
                self.apply_expression_date_constructor(function, &values[..arguments.len()])
            }
            _ => Err(EvalError::UnsupportedExpression),
        }
    }
}

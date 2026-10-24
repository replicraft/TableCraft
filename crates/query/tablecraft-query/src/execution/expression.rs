use super::context::OuterScope;
use super::declarations::relational;
use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::cell::{into_operator_scalar, into_scalar};
use crate::{
    QueryError,
    plan::{Expr, ReadPlan},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::{
    dates::DateConstructor, financial::FinancialFunction, math::MathFunction, text::TextFunction,
};
use tablecraft_expressions::logic::Truth;
use tablecraft_sql::select::{BinaryOperator as B, Quantifier, UnaryOperator as U};

impl Runtime<'_, '_> {
    pub(super) fn eval_predicate(
        &mut self,
        expr: &Expr,
        env: Environment<'_>,
    ) -> Result<Truth, QueryError> {
        let value = self.eval(expr, env)?;
        Ok(self.evaluator().predicate_truth(&value)?)
    }

    pub(super) fn eval(&mut self, expr: &Expr, env: Environment<'_>) -> Result<Value, QueryError> {
        self.tick()?;
        match expr {
            Expr::Array(_) => Err(QueryError::Unsupported),
            Expr::ValueInspect(function, input) => self.value_inspection(*function, input, env),
            Expr::ArrayBound(function, input, dimension) => {
                self.array_bound(*function, input, dimension, env)
            }
            Expr::Clock(function) => Ok(self.evaluator().apply_clock(*function)?),
            Expr::CurrentUser => Ok(self.evaluator().apply_current_user()?),
            Expr::Literal(literal) => Ok(self.evaluator().evaluate(literal)?),
            Expr::Field { .. } | Expr::CollectionItem { .. } | Expr::Output(..) => {
                Ok(into_scalar(self.expression_operand(expr, env)?)?)
            }
            Expr::Parameter(index) => self.copy(self.parameters[*index]),
            Expr::Unary(U::Plus | U::Minus, _) => {
                Ok(into_scalar(self.expression_operand(expr, env)?)?)
            }
            Expr::Unary(op, e) => {
                let value = self.expression_operand(e, env)?;
                Ok(self
                    .evaluator()
                    .apply_unary(*op, into_operator_scalar(value)?)?)
            }
            Expr::Binary(
                B::Add
                | B::Subtract
                | B::Multiply
                | B::Divide
                | B::IntegerDivide
                | B::Modulo
                | B::Power
                | B::Concatenate,
                _,
                _,
            ) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::Binary(op, left, right) => {
                let a = self.expression_operand(left, env)?;
                let b = self.expression_operand(right, env)?;
                let comparison = relational::operator(*op);
                if let Some(comparison) = comparison {
                    let policy = self.expression_pair_policy(left, right, &a, &b)?;
                    return Ok(self
                        .compare_expression_predicate_policy(comparison, &a, &b, policy)?
                        .into_value());
                }
                Ok(self
                    .evaluator()
                    .apply_expression_binary(*op, a.value, b.value)?)
            }
            Expr::Like(left, right, negated) => {
                into_scalar(self.declared_like(left, right, *negated, env)?).map_err(Into::into)
            }
            Expr::IIf(..) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::DateAdd(interval, number, date) => {
                let interval = self.expression_operand(interval, env)?.value;
                let number = self.expression_operand(number, env)?.value;
                let date = self.expression_operand(date, env)?.value;
                Ok(self
                    .evaluator()
                    .apply_expression_date_add(&interval, &number, &date)?)
            }
            Expr::DateDiff(interval, first, second, options) => {
                let interval = self.expression_operand(interval, env)?.value;
                let first = self.expression_operand(first, env)?.value;
                let second = self.expression_operand(second, env)?.value;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let Some(expr) = argument {
                        *setting = Some(self.expression_operand(expr, env)?.value);
                    }
                }
                Ok(self.evaluator().apply_expression_date_diff(
                    &interval,
                    &first,
                    &second,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )?)
            }
            Expr::DatePart(interval, date, options) => {
                let interval = self.expression_operand(interval, env)?.value;
                let date = self.expression_operand(date, env)?.value;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let Some(expr) = argument {
                        *setting = Some(self.expression_operand(expr, env)?.value);
                    }
                }
                Ok(self.evaluator().apply_expression_date_part(
                    &interval,
                    &date,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )?)
            }
            Expr::Weekday(value, first_day) => {
                let value = self.expression_operand(value, env)?.value;
                let first_day = first_day
                    .as_ref()
                    .map(|expr| self.expression_operand(expr, env).map(|cell| cell.value))
                    .transpose()?;
                Ok(self
                    .evaluator()
                    .apply_expression_weekday(&value, first_day.as_ref())?)
            }
            Expr::GuidFromString(expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_guid_from_string(&value)?)
            }
            Expr::StringFromGuid(expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_string_from_guid(&value)?)
            }
            Expr::ErrorMessage(expr) | Expr::ErrorMessageString(expr) => {
                let value = self.expression_operand(expr, env)?.value;
                Ok(self.evaluator().apply_expression_error_message(&value)?)
            }
            Expr::CVar(_) | Expr::CvErr(_) | Expr::Eval(_) => {
                Ok(into_scalar(self.expression_operand(expr, env)?)?)
            }
            Expr::Convert(conversion, expr) => {
                let value = self.expression_operand(expr, env)?;
                Ok(self
                    .evaluator()
                    .apply_expression_conversion(*conversion, &value)?)
            }
            Expr::DateConvert(function, expr) => {
                let value = self.expression_operand(expr, env)?.value;
                Ok(self
                    .evaluator()
                    .apply_expression_date_conversion(*function, &value)?)
            }
            Expr::Inspect(function, expr) => {
                let value = self.expression_operand(expr, env)?.value;
                Ok(self
                    .evaluator()
                    .apply_expression_inspection(*function, &value)?)
            }
            Expr::DateCall(function, expr) => {
                let value = self.expression_operand(expr, env)?.value;
                Ok(self
                    .evaluator()
                    .apply_expression_date_function(*function, &value)?)
            }
            Expr::DomainCall(function, arguments) => self.domain(*function, arguments, env),
            Expr::ConditionalCall(_, _) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::CashFlowCall(function, operands, array) => {
                self.cashflows(*function, operands, array, env)
            }
            Expr::FinancialCall(function, arguments) => {
                let mut values: [ExpressionValue; FinancialFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.expression_operand(argument, env)?.value;
                }
                Ok(self
                    .evaluator()
                    .apply_expression_financial(*function, &values[..arguments.len()])?)
            }
            Expr::MathCall(function, arguments) => {
                let mut values: [ExpressionValue; MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.expression_operand(argument, env)?.value;
                }
                Ok(self
                    .evaluator()
                    .apply_expression_math(*function, &values[..arguments.len()])?)
            }
            Expr::ConstructorCall(function, arguments) => {
                let mut values: [ExpressionValue; DateConstructor::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.expression_operand(argument, env)?.value;
                }
                Ok(self
                    .evaluator()
                    .apply_expression_date_constructor(*function, &values[..arguments.len()])?)
            }
            Expr::TextCall(function, arguments) => {
                if *function == TextFunction::CStr {
                    let value = self.expression_operand(&arguments[0], env)?;
                    return Ok(self.evaluator().apply_expression_cstr(&value)?);
                }
                if *function == TextFunction::Str {
                    let value = self.expression_operand(&arguments[0], env)?;
                    return Ok(self.evaluator().apply_expression_str(&value)?);
                }
                if matches!(function, TextFunction::Hex | TextFunction::Oct) {
                    let value = self.expression_operand(&arguments[0], env)?;
                    return Ok(self.evaluator().apply_expression_radix(*function, &value)?);
                }
                if matches!(
                    function,
                    TextFunction::Len
                        | TextFunction::LTrim
                        | TextFunction::RTrim
                        | TextFunction::Trim
                        | TextFunction::LCase
                        | TextFunction::UCase
                        | TextFunction::Asc
                        | TextFunction::AscW
                        | TextFunction::StrReverse
                        | TextFunction::Val
                ) {
                    let value = self.expression_operand(&arguments[0], env)?;
                    return Ok(self
                        .evaluator()
                        .apply_expression_string_function(*function, &value)?);
                }
                if *function == TextFunction::InStrB {
                    return self.byte_search_call(arguments, env);
                }
                if matches!(
                    function,
                    TextFunction::Chr | TextFunction::ChrString | TextFunction::ChrW
                ) {
                    let value = self.expression_operand(&arguments[0], env)?;
                    return Ok(self
                        .evaluator()
                        .apply_expression_character(*function, &value)?);
                }
                if function.produces_bytes() {
                    let operand = self.byte_call(*function, arguments, env)?;
                    return Ok(self.evaluator().publish_byte_operand(operand)?);
                }
                if matches!(function, TextFunction::LenB | TextFunction::AscB) {
                    let operand = self.expression_byte_operand(&arguments[0], env)?;
                    return Ok(self
                        .evaluator()
                        .observe_expression_byte_operand(*function, operand)?);
                }
                if matches!(
                    function,
                    TextFunction::Left
                        | TextFunction::Right
                        | TextFunction::Mid
                        | TextFunction::InStr
                        | TextFunction::InStrRev
                        | TextFunction::Replace
                        | TextFunction::StrComp
                        | TextFunction::Space
                        | TextFunction::String
                        | TextFunction::FormatNumber
                        | TextFunction::FormatPercent
                        | TextFunction::FormatCurrency
                        | TextFunction::FormatDateTime
                        | TextFunction::MonthName
                        | TextFunction::WeekdayName
                        | TextFunction::Format
                ) {
                    let mut values: [ExpressionValue; TextFunction::MAX_ARGUMENTS] =
                        std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
                    for (argument, value) in arguments.iter().zip(&mut values) {
                        *value = self.expression_operand(argument, env)?.value;
                    }
                    let values = &values[..arguments.len()];
                    let value = match function {
                        TextFunction::InStr | TextFunction::InStrRev => self
                            .evaluator()
                            .apply_expression_search(*function, values)?,
                        TextFunction::Replace => {
                            self.evaluator().apply_expression_replacement(values)?
                        }
                        TextFunction::StrComp => {
                            self.evaluator().apply_expression_comparison(values)?
                        }
                        TextFunction::Space | TextFunction::String => self
                            .evaluator()
                            .apply_expression_repeat(*function, values)?,
                        TextFunction::FormatNumber
                        | TextFunction::FormatPercent
                        | TextFunction::FormatCurrency => self
                            .evaluator()
                            .apply_expression_numeric_format(*function, values)?,
                        TextFunction::FormatDateTime => {
                            self.evaluator().apply_expression_date_format(values)?
                        }
                        TextFunction::MonthName | TextFunction::WeekdayName => self
                            .evaluator()
                            .apply_expression_calendar_name(*function, values)?,
                        TextFunction::Format => self.evaluator().apply_expression_format(values)?,
                        _ => self.evaluator().apply_expression_slice(*function, values)?,
                    };
                    return Ok(value);
                }
                let mut values: [Value; TextFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(self
                    .evaluator()
                    .apply_text_function(*function, &values[..arguments.len()])?)
            }
            Expr::Nz(..) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::IsNull(e, negated) => {
                let value = self.expression_operand(e, env)?;
                Ok(Value::Boolean(
                    matches!(value.value, ExpressionValue::Value(Value::Null)) != *negated,
                ))
            }
            Expr::Between(expression, first, second, negated) => {
                into_scalar(self.declared_range(expression, first, second, *negated, env)?)
                    .map_err(Into::into)
            }
            Expr::InList(expression, list, negated) => {
                into_scalar(self.declared_membership(expression, list, *negated, env)?)
                    .map_err(Into::into)
            }
            Expr::Aggregate(kind, argument) => self.aggregate(*kind, argument.as_deref(), env),
            Expr::Subquery(plan) => {
                let rows = self.nested(plan, env)?;
                match rows.len() {
                    0 => Ok(Value::Null),
                    1 => Ok(into_scalar(self.copy_expression(&rows[0][0])?)?),
                    _ => Err(QueryError::Cardinality),
                }
            }
            Expr::Exists(plan) => Ok(Value::Boolean(!self.nested(plan, env)?.is_empty())),
            Expr::InQuery(expression, plan, negated) => {
                into_scalar(self.declared_subquery_predicate(
                    expression,
                    B::Equal,
                    Quantifier::Any,
                    *negated,
                    plan,
                    env,
                )?)
                .map_err(Into::into)
            }
            Expr::Quantified(expression, operator, quantifier, plan) => {
                into_scalar(self.declared_subquery_predicate(
                    expression,
                    *operator,
                    *quantifier,
                    false,
                    plan,
                    env,
                )?)
                .map_err(Into::into)
            }
        }
    }
    pub(super) fn nested(
        &mut self,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<Vec<Vec<CellValue>>, QueryError> {
        self.tick()?;
        let count = env
            .outer
            .len()
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        self.budget
            .charge_allocation(count as u64, size_of::<OuterScope<'_>>() as u64)?;
        let mut outer = Vec::new();
        outer
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        outer.push(OuterScope {
            row: env.row,
            group_keys: env.group_keys,
        });
        outer.extend_from_slice(env.outer);
        self.read(plan, &outer)
    }
}

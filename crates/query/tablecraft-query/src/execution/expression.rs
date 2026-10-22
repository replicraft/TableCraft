use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Expr, ReadPlan},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::{
    conversions::errors::into_scalar, dates::DateConstructor, financial::FinancialFunction,
    math::MathFunction, text::TextFunction,
};
use tablecraft_expressions::{comparison::Comparison, logic::Truth, predicates::Membership};
use tablecraft_sql::select::{BinaryOperator as B, Quantifier};

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
            Expr::Unary(op, e) => {
                let v = self.eval(e, env)?;
                Ok(self.evaluator().apply_unary(*op, v)?)
            }
            Expr::Binary(op, left, right) => {
                let a = self.eval(left, env)?;
                let b = self.eval(right, env)?;
                let policy = if matches!(
                    op,
                    B::Equal
                        | B::NotEqual
                        | B::Less
                        | B::LessOrEqual
                        | B::Greater
                        | B::GreaterOrEqual
                ) {
                    self.text_pair_policy(left, right, &a, &b)?
                } else {
                    crate::source::TextPolicy::Default
                };
                let collation = self.comparator(policy, &a, &b)?;
                let evaluator = self.evaluator();
                let mut evaluator = if let Some(c) = collation {
                    evaluator.with_collation(c)
                } else {
                    evaluator
                };
                Ok(evaluator.apply_binary(*op, &a, &b)?)
            }
            Expr::Like(left, right, negated) => {
                let a = self.eval(left, env)?;
                let b = self.eval(right, env)?;
                let policy = if self.options.like_mode.is_some() {
                    self.text_pair_policy(left, right, &a, &b)?
                } else {
                    crate::source::TextPolicy::Default
                };
                let collation = self.comparator(policy, &a, &b)?;
                let evaluator = self.evaluator();
                let mut evaluator = if let Some(c) = collation {
                    evaluator.with_collation(c)
                } else {
                    evaluator
                };
                Ok(evaluator.apply_like(&a, &b, *negated)?)
            }
            Expr::IIf(..) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::DateAdd(interval, number, date) => {
                let interval = self.eval(interval, env)?;
                let number = self.eval(number, env)?;
                let date = self.eval(date, env)?;
                Ok(self.evaluator().apply_date_add(&interval, &number, &date)?)
            }
            Expr::DateDiff(interval, first, second, options) => {
                let interval = self.eval(interval, env)?;
                let first = self.eval(first, env)?;
                let second = self.eval(second, env)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let Some(expr) = argument {
                        *setting = Some(self.eval(expr, env)?);
                    }
                }
                Ok(self.evaluator().apply_date_diff(
                    &interval,
                    &first,
                    &second,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )?)
            }
            Expr::DatePart(interval, date, options) => {
                let interval = self.eval(interval, env)?;
                let date = self.eval(date, env)?;
                let mut settings = [None, None];
                for (argument, setting) in options.iter().zip(&mut settings) {
                    if let Some(expr) = argument {
                        *setting = Some(self.eval(expr, env)?);
                    }
                }
                Ok(self.evaluator().apply_date_part(
                    &interval,
                    &date,
                    settings[0].as_ref(),
                    settings[1].as_ref(),
                )?)
            }
            Expr::Weekday(value, first_day) => {
                let value = self.eval(value, env)?;
                let first_day = first_day
                    .as_ref()
                    .map(|expr| self.eval(expr, env))
                    .transpose()?;
                Ok(self.evaluator().apply_weekday(&value, first_day.as_ref())?)
            }
            Expr::GuidFromString(expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_guid_from_string(&value)?)
            }
            Expr::StringFromGuid(expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_string_from_guid(&value)?)
            }
            Expr::ErrorMessage(expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_error_message(&value)?)
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
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_date_conversion(*function, &value)?)
            }
            Expr::Inspect(function, expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_inspection(*function, &value)?)
            }
            Expr::DateCall(function, expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_date_function(*function, &value)?)
            }
            Expr::DomainCall(function, arguments) => self.domain(*function, arguments, env),
            Expr::ConditionalCall(_, _) => Ok(into_scalar(self.expression_operand(expr, env)?)?),
            Expr::CashFlowCall(function, operands, array) => {
                self.cashflows(*function, operands, array, env)
            }
            Expr::FinancialCall(function, arguments) => {
                let mut values: [Value; FinancialFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(self
                    .evaluator()
                    .apply_financial(*function, &values[..arguments.len()])?)
            }
            Expr::MathCall(function, arguments) => {
                let mut values: [Value; MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(self
                    .evaluator()
                    .apply_math(*function, &values[..arguments.len()])?)
            }
            Expr::ConstructorCall(function, arguments) => {
                let mut values: [Value; DateConstructor::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(self
                    .evaluator()
                    .apply_date_constructor(*function, &values[..arguments.len()])?)
            }
            Expr::TextCall(function, arguments) => {
                if *function == TextFunction::InStrB {
                    return self.byte_search_call(arguments, env);
                }
                if function.produces_bytes() {
                    let operand = self.byte_call(*function, arguments, env)?;
                    return Ok(self.evaluator().publish_byte_operand(operand)?);
                }
                if matches!(function, TextFunction::LenB | TextFunction::AscB) {
                    let operand = self.byte_operand(&arguments[0], env)?;
                    return Ok(self.evaluator().observe_byte_operand(*function, operand)?);
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
                    matches!(value, ExpressionValue::Value(Value::Null)) != *negated,
                ))
            }
            Expr::Between(e, first, second, negated) => {
                let v = self.eval(e, env)?;
                let a = self.eval(first, env)?;
                let b = self.eval(second, env)?;
                let first_policy = self.text_pair_policy(e, first, &v, &a)?;
                let second_policy = self.text_pair_policy(e, second, &v, &b)?;
                let forward = self
                    .compare_policy(Comparison::GreaterOrEqual, &v, &a, first_policy)?
                    .and(self.compare_policy(Comparison::LessOrEqual, &v, &b, second_policy)?);
                let reverse = self
                    .compare_policy(Comparison::GreaterOrEqual, &v, &b, second_policy)?
                    .and(self.compare_policy(Comparison::LessOrEqual, &v, &a, first_policy)?);
                Ok(negate(forward.or(reverse), *negated).into_value())
            }
            Expr::InList(e, list, negated) => {
                let v = self.eval(e, env)?;
                let mut membership = Membership::new();
                for expression in list {
                    let item = self.eval(expression, env)?;
                    let policy = self.text_pair_policy(e, expression, &v, &item)?;
                    membership.consider(&v, &item, |a, b| {
                        self.compare_policy(Comparison::Equal, a, b, policy)
                    })?;
                }
                Ok(membership.finish(*negated).into_value())
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
            Expr::InQuery(e, plan, negated) => {
                let v = self.eval(e, env)?;
                let rows = self.nested(plan, env)?;
                let mut truth = Truth::False;
                for row in rows {
                    let item = super::values::scalar(&row[0])?;
                    let policy = if matches!((&v, item), (Value::Text(_), Value::Text(_))) {
                        self.text_policy(e)?
                            .merge(*plan.text_policies.first().ok_or(QueryError::Width)?)
                    } else {
                        crate::source::TextPolicy::Default
                    };
                    truth = truth.or(self.compare_policy(Comparison::Equal, &v, item, policy)?);
                }
                Ok(negate(truth, *negated).into_value())
            }
            Expr::Quantified(e, op, q, plan) => {
                let v = self.eval(e, env)?;
                let rows = self.nested(plan, env)?;
                let all = *q == Quantifier::All;
                let mut truth = if all { Truth::True } else { Truth::False };
                let comparison = match op {
                    B::Equal => Comparison::Equal,
                    B::NotEqual => Comparison::NotEqual,
                    B::Less => Comparison::Less,
                    B::LessOrEqual => Comparison::LessOrEqual,
                    B::Greater => Comparison::Greater,
                    B::GreaterOrEqual => Comparison::GreaterOrEqual,
                    _ => return Err(QueryError::Unsupported),
                };
                for row in rows {
                    let item = super::values::scalar(&row[0])?;
                    let policy = if matches!((&v, item), (Value::Text(_), Value::Text(_))) {
                        self.text_policy(e)?
                            .merge(*plan.text_policies.first().ok_or(QueryError::Width)?)
                    } else {
                        crate::source::TextPolicy::Default
                    };
                    let item = self.compare_policy(comparison, &v, item, policy)?;
                    truth = if all { truth.and(item) } else { truth.or(item) };
                }
                Ok(truth.into_value())
            }
        }
    }
    pub(super) fn nested(
        &mut self,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<Vec<Vec<ExpressionValue>>, QueryError> {
        let mut outer = Vec::new();
        outer
            .try_reserve_exact(env.outer.len() + 1)
            .map_err(|_| QueryError::Allocation)?;
        outer.push(env.row);
        outer.extend_from_slice(env.outer);
        self.read(plan, &outer)
    }
}
fn negate(truth: Truth, negated: bool) -> Truth {
    if negated { truth.negate() } else { truth }
}

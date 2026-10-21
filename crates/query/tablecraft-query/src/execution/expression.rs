use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Expr, ReadPlan},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::dates::TwoDigitYearWindow;
use tablecraft_expressions::{
    comparison::{self, Comparison},
    logic::Truth,
    predicates::{self, Membership},
};
use tablecraft_sql::select::{BinaryOperator as B, Quantifier};

impl Runtime<'_> {
    pub(super) fn eval(&mut self, expr: &Expr, env: Environment<'_>) -> Result<Value, QueryError> {
        self.tick()?;
        match expr {
            Expr::Clock(function) => Ok(self.evaluator().apply_clock(*function)?),
            Expr::Literal(literal) => Ok(self.evaluator().evaluate(literal)?),
            Expr::Field { depth, index, .. } => {
                let row = if *depth == 0 {
                    env.row
                } else {
                    env.outer
                        .get(depth - 1)
                        .copied()
                        .ok_or(QueryError::UnknownName)?
                };
                self.copy(row.get(*index).ok_or(QueryError::UnknownName)?)
            }
            Expr::CollectionItem {
                depth,
                index,
                width,
            } => {
                let item = if *depth == 0 {
                    env.output.get(*index)
                } else {
                    env.outer.get(depth - 1).and_then(|row| {
                        row.len()
                            .checked_sub(*width)
                            .and_then(|start| start.checked_add(*index))
                            .and_then(|position| row.get(position))
                    })
                };
                self.copy(item.ok_or(QueryError::UnknownName)?)
            }
            Expr::Parameter(index) => self.copy(self.parameters[*index]),
            Expr::Output(index, _) => {
                self.copy(env.output.get(*index).ok_or(QueryError::UnknownName)?)
            }
            Expr::Unary(op, e) => {
                let v = self.eval(e, env)?;
                Ok(self.evaluator().apply_unary(*op, v)?)
            }
            Expr::Binary(op, a, b) => {
                let a = self.eval(a, env)?;
                let b = self.eval(b, env)?;
                Ok(self.evaluator().apply_binary(*op, &a, &b)?)
            }
            Expr::Like(a, b, negated) => {
                let a = self.eval(a, env)?;
                let b = self.eval(b, env)?;
                Ok(self.evaluator().apply_like(&a, &b, *negated)?)
            }
            Expr::IIf(condition, truepart, falsepart) => {
                let condition = self.eval(condition, env)?;
                let truepart = self.eval(truepart, env)?;
                let falsepart = self.eval(falsepart, env)?;
                Ok(tablecraft_expressions::functions::iif(
                    &condition, truepart, falsepart,
                )?)
            }
            Expr::DateAdd(interval, number, date) => {
                let interval = self.eval(interval, env)?;
                let number = self.eval(number, env)?;
                let date = self.eval(date, env)?;
                Ok(tablecraft_expressions::functions::dates::date_add(
                    &interval, &number, &date,
                )?)
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
                Ok(
                    tablecraft_expressions::functions::dates::date_diff_with_context(
                        &interval,
                        &first,
                        &second,
                        settings[0].as_ref(),
                        settings[1].as_ref(),
                        tablecraft_expressions::functions::dates::WeekContext {
                            first_day: self.options.first_day_of_week,
                            first_week: self.options.first_week_of_year,
                        },
                    )?,
                )
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
                Ok(
                    tablecraft_expressions::functions::dates::date_part_with_context(
                        &interval,
                        &date,
                        settings[0].as_ref(),
                        settings[1].as_ref(),
                        tablecraft_expressions::functions::dates::WeekContext {
                            first_day: self.options.first_day_of_week,
                            first_week: self.options.first_week_of_year,
                        },
                    )?,
                )
            }
            Expr::Weekday(value, first_day) => {
                let value = self.eval(value, env)?;
                let first_day = first_day
                    .as_ref()
                    .map(|expr| self.eval(expr, env))
                    .transpose()?;
                Ok(self.evaluator().apply_weekday(&value, first_day.as_ref())?)
            }
            Expr::Convert(conversion, expr) => {
                let value = self.eval(expr, env)?;
                Ok(self.evaluator().apply_conversion(*conversion, &value)?)
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
                Ok(function.apply(&value)?)
            }
            Expr::ConditionalCall(function, arguments) => {
                let mut selection = function.start(arguments.len())?;
                for argument in arguments {
                    selection.push(self.eval(argument, env)?)?;
                }
                Ok(selection.finish()?)
            }
            Expr::MathCall(function, arguments) => {
                let mut values: [Value;
                    tablecraft_expressions::functions::math::MathFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(function.apply(&values[..arguments.len()])?)
            }
            Expr::ConstructorCall(function, arguments) => {
                let mut values: [Value;
                    tablecraft_expressions::functions::dates::DateConstructor::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(function.apply_with_window(
                    &values[..arguments.len()],
                    self.options
                        .year_window
                        .unwrap_or(TwoDigitYearWindow::ACCESS_DEFAULT),
                )?)
            }
            Expr::TextCall(function, arguments) => {
                let mut values: [Value;
                    tablecraft_expressions::functions::text::TextFunction::MAX_ARGUMENTS] =
                    std::array::from_fn(|_| Value::Null);
                for (argument, value) in arguments.iter().zip(&mut values) {
                    *value = self.eval(argument, env)?;
                }
                Ok(self
                    .evaluator()
                    .apply_text_function(*function, &values[..arguments.len()])?)
            }
            Expr::Nz(value, replacement) => {
                let value = self.eval(value, env)?;
                let replacement = replacement
                    .as_ref()
                    .map(|expr| self.eval(expr, env))
                    .transpose()?;
                Ok(tablecraft_expressions::functions::nz_query(
                    value,
                    replacement,
                ))
            }
            Expr::IsNull(e, negated) => {
                let value = self.eval(e, env)?;
                Ok(Value::Boolean(
                    tablecraft_expressions::functions::is_null(&value) != *negated,
                ))
            }
            Expr::Between(e, a, b, negated) => {
                let v = self.eval(e, env)?;
                let a = self.eval(a, env)?;
                let b = self.eval(b, env)?;
                Ok(
                    predicates::between(&v, &a, &b, *negated, |op, a, b| self.compare(op, a, b))?
                        .into_value(),
                )
            }
            Expr::InList(e, list, negated) => {
                let v = self.eval(e, env)?;
                let mut membership = Membership::new();
                for item in list {
                    let item = self.eval(item, env)?;
                    membership.consider(&v, &item, |a, b| self.compare(Comparison::Equal, a, b))?;
                }
                Ok(membership.finish(*negated).into_value())
            }
            Expr::Aggregate(kind, argument) => self.aggregate(*kind, argument.as_deref(), env),
            Expr::Subquery(plan) => {
                let rows = self.nested(plan, env)?;
                match rows.len() {
                    0 => Ok(Value::Null),
                    1 => self.copy(&rows[0][0]),
                    _ => Err(QueryError::Cardinality),
                }
            }
            Expr::Exists(plan) => Ok(Value::Boolean(!self.nested(plan, env)?.is_empty())),
            Expr::InQuery(e, plan, negated) => {
                let v = self.eval(e, env)?;
                let rows = self.nested(plan, env)?;
                let mut truth = Truth::False;
                for row in rows {
                    truth = truth.or(self.compare(Comparison::Equal, &v, &row[0])?);
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
                    let item = self.compare(comparison, &v, &row[0])?;
                    truth = if all { truth.and(item) } else { truth.or(item) };
                }
                Ok(truth.into_value())
            }
        }
    }
    fn compare(&mut self, op: Comparison, a: &Value, b: &Value) -> Result<Truth, QueryError> {
        self.tick()?;
        Ok(comparison::compare(op, a, b, self.options.collation)?)
    }
    fn nested(
        &mut self,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<Vec<Vec<Value>>, QueryError> {
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

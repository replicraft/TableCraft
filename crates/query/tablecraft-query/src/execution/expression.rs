use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Expr, ReadPlan},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    comparison::{self, Comparison},
    logic::Truth,
};
use tablecraft_sql::select::{BinaryOperator as B, Quantifier};

impl Runtime<'_> {
    pub(super) fn eval(&mut self, expr: &Expr, env: Environment<'_>) -> Result<Value, QueryError> {
        self.tick()?;
        match expr {
            Expr::Literal(literal) => Ok(self.evaluator().evaluate(literal)?),
            Expr::Field { depth, index } => {
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
            Expr::Parameter(index) => self.copy(self.parameters[*index]),
            Expr::Output(index) => {
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
            Expr::IsNull(e, negated) => Ok(Value::Boolean(
                matches!(self.eval(e, env)?, Value::Null) != *negated,
            )),
            Expr::Between(e, a, b, negated) => {
                let v = self.eval(e, env)?;
                let a = self.eval(a, env)?;
                let b = self.eval(b, env)?;
                let forward = self
                    .compare(Comparison::GreaterOrEqual, &v, &a)?
                    .and(self.compare(Comparison::LessOrEqual, &v, &b)?);
                let reverse = self
                    .compare(Comparison::GreaterOrEqual, &v, &b)?
                    .and(self.compare(Comparison::LessOrEqual, &v, &a)?);
                Ok(negate(forward.or(reverse), *negated).into_value())
            }
            Expr::InList(e, list, negated) => {
                let v = self.eval(e, env)?;
                let mut truth = Truth::False;
                for item in list {
                    let item = self.eval(item, env)?;
                    truth = truth.or(self.compare(Comparison::Equal, &v, &item)?);
                }
                Ok(negate(truth, *negated).into_value())
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

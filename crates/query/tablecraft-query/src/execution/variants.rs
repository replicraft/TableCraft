//! Expression-only Error data has explicit consumers and cannot become stored cells.
use super::{Environment, Runtime};
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::conditional::expression as selection;

impl Runtime<'_> {
    pub(super) fn expression_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionValue, QueryError> {
        match expression {
            Expr::Eval(value) => {
                self.tick()?;
                let value = self.eval(value, env)?;
                Ok(self.evaluator().apply_eval_expression(&value)?)
            }
            Expr::Aggregate(kind @ (Aggregate::First | Aggregate::Last), Some(argument)) => {
                self.tick()?;
                self.encounter_aggregate(argument, env, matches!(kind, Aggregate::Last))
            }
            Expr::DomainCall(kind, arguments) => {
                self.tick()?;
                self.domain_expression(*kind, arguments, env)
            }
            Expr::Field { depth, index, .. } => {
                self.tick()?;
                let row = if *depth == 0 {
                    env.row
                } else {
                    env.outer
                        .get(depth - 1)
                        .copied()
                        .ok_or(QueryError::UnknownName)?
                };
                self.copy_expression(row.get(*index).ok_or(QueryError::UnknownName)?)
            }
            Expr::Output(index, _) => {
                self.tick()?;
                self.copy_expression(env.output.get(*index).ok_or(QueryError::UnknownName)?)
            }
            Expr::CollectionItem {
                depth,
                index,
                width,
                ..
            } => {
                self.tick()?;
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
                self.copy_expression(item.ok_or(QueryError::UnknownName)?)
            }
            Expr::Subquery(plan) => {
                self.tick()?;
                let rows = self.nested(plan, env)?;
                match rows.len() {
                    0 => Ok(ExpressionValue::Value(Value::Null)),
                    1 => self.copy_expression(&rows[0][0]),
                    _ => Err(QueryError::Cardinality),
                }
            }
            Expr::CvErr(value) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                Ok(self.evaluator().apply_cverr(value)?)
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_operand(value, env)
            }
            Expr::IIf(condition, yes, no) => {
                self.tick()?;
                let condition = self.expression_operand(condition, env)?;
                let yes = self.expression_operand(yes, env)?;
                let no = self.expression_operand(no, env)?;
                Ok(self.evaluator().apply_iif(&condition, yes, no)?)
            }
            Expr::Nz(value, replacement) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                let replacement = replacement
                    .as_ref()
                    .map(|replacement| self.expression_operand(replacement, env))
                    .transpose()?;
                Ok(selection::nz(value, replacement))
            }
            Expr::ConditionalCall(function, arguments) => {
                self.tick()?;
                let mut selection = function.start(arguments.len())?;
                for argument in arguments {
                    let value = self.expression_operand(argument, env)?;
                    self.evaluator()
                        .push_selection_argument(&mut selection, value)?;
                }
                Ok(selection.finish_expression()?)
            }
            _ => self.eval(expression, env).map(ExpressionValue::Value),
        }
    }
}

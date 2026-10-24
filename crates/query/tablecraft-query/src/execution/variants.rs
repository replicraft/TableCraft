//! Expression-only Error data has explicit consumers and cannot become stored cells.
use super::declarations::relational;
use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::{
    QueryError,
    plan::{Aggregate, Expr},
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{functions::conversions::Conversion, numeric::OperandDeclaration};
use tablecraft_sql::select::{BinaryOperator, Quantifier};

impl Runtime<'_, '_> {
    pub(super) fn expression_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let value = self.expression_cell(expression, env)?;
        let policy = self.result_text_policy(expression, value.policy, env)?;
        Ok(value.with_policy(policy))
    }

    fn expression_cell(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        match expression {
            Expr::Eval(value) => {
                self.tick()?;
                let value = self.eval(value, env)?;
                Ok(self.evaluator().apply_eval_expression(&value)?.into())
            }
            Expr::Aggregate(kind @ (Aggregate::First | Aggregate::Last), Some(argument)) => {
                self.tick()?;
                self.encounter_aggregate(argument, env, matches!(kind, Aggregate::Last))
                    .map(|value| value.with_declaration(None))
            }
            Expr::Aggregate(kind @ (Aggregate::Minimum | Aggregate::Maximum), Some(argument)) => {
                self.tick()?;
                self.extreme_aggregate(argument, env, matches!(kind, Aggregate::Maximum))
                    .map(|value| value.with_declaration(None))
            }
            Expr::DomainCall(kind, arguments) => {
                self.tick()?;
                let declaration = super::declarations::functions::result(expression);
                self.domain_expression(*kind, arguments, env)
                    .map(|value| value.with_declaration(declaration))
            }
            Expr::Like(left, right, negated) => {
                self.tick()?;
                self.declared_like(left, right, *negated, env)
            }
            Expr::Between(expression, first, second, negated) => {
                self.tick()?;
                self.declared_range(expression, first, second, *negated, env)
            }
            Expr::InList(expression, list, negated) => {
                self.tick()?;
                self.declared_membership(expression, list, *negated, env)
            }
            Expr::InQuery(expression, plan, negated) => {
                self.tick()?;
                self.declared_subquery_predicate(
                    expression,
                    BinaryOperator::Equal,
                    Quantifier::Any,
                    *negated,
                    plan,
                    env,
                )
            }
            Expr::Quantified(expression, operator, quantifier, plan) => {
                self.tick()?;
                self.declared_subquery_predicate(
                    expression,
                    *operator,
                    *quantifier,
                    false,
                    plan,
                    env,
                )
            }
            Expr::Field { depth, index, .. } => {
                self.tick()?;
                let row = if *depth == 0 {
                    env.row
                } else {
                    env.outer
                        .get(depth - 1)
                        .map(|scope| scope.row)
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
                    env.outer.get(depth - 1).and_then(|scope| {
                        let row = scope.row;
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
                let declaration = *plan.declarations.first().ok_or(QueryError::Width)?;
                let rows = self.nested(plan, env)?;
                match rows.len() {
                    0 => Ok(CellValue::scalar(Value::Null).with_declaration(declaration)),
                    1 => self
                        .copy_expression(&rows[0][0])
                        .map(|value| value.with_declaration(declaration)),
                    _ => Err(QueryError::Cardinality),
                }
            }
            Expr::Binary(BinaryOperator::Concatenate, left, right) => {
                self.concatenation_cell(left, right, env)
            }
            Expr::Binary(BinaryOperator::Add, left, right) => self.addition_cell(left, right, env),
            Expr::Binary(operator, left, right)
                if let Some(comparison) = relational::operator(*operator) =>
            {
                self.tick()?;
                let left_value = self.expression_operand(left, env)?;
                let right_value = self.expression_operand(right, env)?;
                self.declared_comparison(comparison, left, right, left_value, right_value)
            }
            Expr::Binary(operator, left, right)
                if matches!(
                    operator,
                    BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::IntegerDivide
                        | BinaryOperator::Modulo
                        | BinaryOperator::Power
                ) =>
            {
                self.numeric_arithmetic_cell(*operator, left, right, env)
            }
            Expr::Binary(operator, left, right)
                if matches!(
                    operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Xor
                        | BinaryOperator::Equivalence
                        | BinaryOperator::Implication
                ) =>
            {
                self.tick()?;
                let left = self.expression_operand(left, env)?;
                let right = self.expression_operand(right, env)?;
                self.declared_binary(*operator, left, right)
            }
            Expr::Parameter(index) => {
                let declaration = self.parameters[*index].declaration;
                self.eval(expression, env)
                    .map(|value| CellValue::scalar(value).with_declaration(Some(declaration)))
            }
            Expr::Literal(_) | Expr::DateLiteral(..) => self.eval(expression, env).map(|value| {
                CellValue::scalar(value).with_declaration(Some(OperandDeclaration::Scalar))
            }),
            Expr::Convert(conversion, _) => {
                let declaration = if *conversion == Conversion::Decimal {
                    OperandDeclaration::Variant
                } else {
                    OperandDeclaration::Scalar
                };
                self.eval(expression, env)
                    .map(|value| CellValue::scalar(value).with_declaration(Some(declaration)))
            }
            Expr::Unary(operator, operand) => {
                self.tick()?;
                let operand = self.expression_operand(operand, env)?;
                self.declared_unary(*operator, operand)
            }
            Expr::CvErr(value) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                Ok(CellValue::from(self.evaluator().apply_cverr(value.value)?)
                    .with_declaration(Some(OperandDeclaration::Variant)))
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_operand(value, env)
                    .map(|value| value.with_declaration(Some(OperandDeclaration::Variant)))
            }
            Expr::IIf(condition, yes, no) => {
                let value = self.iif_string_cell(condition, yes, no, env)?;
                self.publish_string_cell(value)
            }
            Expr::Nz(value, replacement) => {
                let value = self.nz_string_cell(value, replacement.as_deref(), env)?;
                self.publish_string_cell(value)
            }
            Expr::ConditionalCall(function, arguments) => {
                let value = self.conditional_string_cell(*function, arguments, env)?;
                self.publish_string_cell(value)
            }
            _ => {
                let declaration = super::declarations::functions::result(expression);
                self.eval(expression, env)
                    .map(|value| CellValue::scalar(value).with_declaration(declaration))
            }
        }
    }
}

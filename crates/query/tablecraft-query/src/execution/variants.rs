//! Expression-only Error data has explicit consumers and cannot become stored cells.
use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::{
    QueryError,
    plan::{
        Aggregate, Expr,
        identity::{Comparer, Identity},
    },
    profile::MAX_BIND_DEPTH,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::conditional::{
    ConditionalFunction, expression as selection,
};
use tablecraft_expressions::{functions::conversions::Conversion, numeric::OperandDeclaration};
use tablecraft_sql::select::{BinaryOperator, UnaryOperator};

impl Runtime<'_, '_> {
    pub(super) fn expression_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let value = self.expression_cell(expression, env)?;
        let mut policy = self.text_policy(expression)?;
        if self.options.collation_resolver.is_some() {
            let scopes = std::iter::once((0, env.group_keys)).chain(
                env.outer
                    .iter()
                    .enumerate()
                    .map(|(index, scope)| (index + 1, scope.group_keys)),
            );
            for (shift, keys) in scopes {
                let Some(keys) = keys else {
                    continue;
                };
                for (key, cell) in keys.expressions.iter().zip(keys.values) {
                    let mut tick = |depth| {
                        if depth > MAX_BIND_DEPTH {
                            return Err(QueryError::StructureLimit);
                        }
                        self.tick()
                    };
                    if (Comparer { tick: &mut tick }).same_expression(
                        expression,
                        key,
                        1,
                        Identity {
                            shift,
                            projection: false,
                        },
                    )? {
                        policy = policy.merge(cell.policy);
                    }
                }
            }
        }
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
                self.domain_expression(*kind, arguments, env)
                    .map(|value| value.with_declaration(None))
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
                let rows = self.nested(plan, env)?;
                match rows.len() {
                    0 => Ok(CellValue::scalar(Value::Null)),
                    1 => self.copy_expression(&rows[0][0]),
                    _ => Err(QueryError::Cardinality),
                }
            }
            Expr::Binary(operator, left, right)
                if matches!(
                    operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::IntegerDivide
                        | BinaryOperator::Modulo
                        | BinaryOperator::Power
                        | BinaryOperator::Concatenate
                ) =>
            {
                self.tick()?;
                let left = self.expression_operand(left, env)?;
                let right = self.expression_operand(right, env)?;
                self.declared_binary(*operator, left, right)
            }
            Expr::Literal(_) | Expr::Parameter(_) => self.eval(expression, env).map(|value| {
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
            Expr::Unary(operator, operand)
                if matches!(operator, UnaryOperator::Plus | UnaryOperator::Minus) =>
            {
                self.tick()?;
                let operand = self.expression_operand(operand, env)?;
                self.declared_unary(*operator, operand)
            }
            Expr::CvErr(value) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                Ok(self.evaluator().apply_cverr(value.value)?.into())
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_operand(value, env)
                    .map(|value| value.with_declaration(Some(OperandDeclaration::Variant)))
            }
            Expr::IIf(condition, yes, no) => {
                self.tick()?;
                let condition = self.expression_operand(condition, env)?;
                let yes = self.expression_operand(yes, env)?;
                let no = self.expression_operand(no, env)?;
                let policy = yes.policy.merge(no.policy);
                Ok(CellValue::from(
                    self.evaluator()
                        .apply_iif(&condition, yes.value, no.value)?,
                )
                .with_policy(policy)
                .with_declaration(Some(OperandDeclaration::Variant)))
            }
            Expr::Nz(value, replacement) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                let replacement = replacement
                    .as_ref()
                    .map(|replacement| self.expression_operand(replacement, env))
                    .transpose()?;
                let policy = value.policy.merge(
                    replacement
                        .as_ref()
                        .map_or(crate::source::TextPolicy::Default, |value| value.policy),
                );
                Ok(CellValue::from(selection::nz(
                    value.value,
                    replacement.map(|value| value.value),
                ))
                .with_policy(policy)
                .with_declaration(Some(OperandDeclaration::Variant)))
            }
            Expr::ConditionalCall(function, arguments) => {
                self.tick()?;
                let mut selection = function.start(arguments.len())?;
                let mut policy = crate::source::TextPolicy::Default;
                for (index, argument) in arguments.iter().enumerate() {
                    let value = self.expression_operand(argument, env)?;
                    let is_value = match function {
                        ConditionalFunction::Choose => index > 0,
                        ConditionalFunction::Switch => index % 2 == 1,
                    };
                    if is_value {
                        policy = policy.merge(value.policy);
                    }
                    self.evaluator()
                        .push_selection_argument(&mut selection, value.value)?;
                }
                Ok(CellValue::from(selection.finish_expression()?)
                    .with_policy(policy)
                    .with_declaration(Some(OperandDeclaration::Variant)))
            }
            _ => {
                let declaration = super::declarations::functions::result(expression);
                self.eval(expression, env)
                    .map(|value| CellValue::scalar(value).with_declaration(declaration))
            }
        }
    }
}

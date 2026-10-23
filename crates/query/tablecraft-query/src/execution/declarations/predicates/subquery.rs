//! Subquery result categories use left provenance, never observed row tags.
use crate::{
    QueryError,
    execution::{Environment, Runtime, cell::CellValue, declarations::relational},
    plan::{Expr, ReadPlan},
    source::TextPolicy,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{logic::Truth, numeric::OperandDeclaration};
use tablecraft_sql::select::{BinaryOperator, Quantifier};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn declared_subquery_predicate(
        &mut self,
        expression: &Expr,
        operator: BinaryOperator,
        quantifier: Quantifier,
        negated: bool,
        plan: &ReadPlan,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let value = self.expression_operand(expression, env)?;
        let declaration = value
            .declaration
            .filter(|category| *category == OperandDeclaration::Variant);
        let rows = self.nested(plan, env)?;
        let comparison = relational::operator(operator).ok_or(QueryError::Unsupported)?;
        let all = quantifier == Quantifier::All;
        let mut truth = if all { Truth::True } else { Truth::False };
        for row in rows {
            let item = row.first().ok_or(QueryError::Width)?;
            let policy = if matches!(
                (&value.value, &item.value),
                (
                    ExpressionValue::Value(Value::Text(_)),
                    ExpressionValue::Value(Value::Text(_))
                )
            ) {
                self.text_policy(expression)?
                    .merge(*plan.text_policies.first().ok_or(QueryError::Width)?)
            } else {
                TextPolicy::Default
            };
            let item =
                self.compare_expression_predicate_policy(comparison, &value, item, policy)?;
            truth = if all { truth.and(item) } else { truth.or(item) };
        }
        let truth = if negated { truth.negate() } else { truth };
        Ok(CellValue::scalar(truth.into_value()).with_declaration(declaration))
    }
}

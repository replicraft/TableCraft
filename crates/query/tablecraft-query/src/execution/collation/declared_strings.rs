//! Declaration-dependent String predicates and conservative unknown-category guards.
use super::{Runtime, predicate_error};
use crate::{QueryError, execution::cell::CellValue, source::TextPolicy};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    comparison::Comparison,
    evaluation::PreparedScalarStringVariantComparison,
    logic::Truth,
    numeric::{ArithmeticDeclarations, OperandDeclaration},
};

impl Runtime<'_, '_> {
    pub(super) fn compare_declared_string_variant(
        &mut self,
        op: Comparison,
        a: &CellValue,
        b: &CellValue,
        policy: TextPolicy,
    ) -> Result<Option<Truth>, QueryError> {
        if self.options.declared_integral {
            if ambiguous_string_numeric(a, b) {
                self.tick()?;
                let numeric = if expression_string(&a.value) {
                    &b.value
                } else {
                    &a.value
                };
                if matches!(numeric,
                    ExpressionValue::Value(Value::Single(value)) if !value.to_float().is_finite()
                ) || matches!(numeric,
                    ExpressionValue::Value(Value::Double(value)) if !value.to_float().is_finite()
                ) {
                    return Err(EvalError::NonFinite.into());
                }
                return Err(QueryError::ArithmeticDeclaration);
            }
            if let Some(result) = self
                .evaluator()
                .compare_scalar_numeric_string(op, &a.value, &b.value, a.declaration, b.declaration)
                .map_err(predicate_error)?
            {
                return Ok(Some(result));
            }
            let scalar_string = match (a.declaration, b.declaration) {
                (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Variant))
                    if expression_string(&a.value) =>
                {
                    Some((a, b))
                }
                (Some(OperandDeclaration::Variant), Some(OperandDeclaration::Scalar))
                    if expression_string(&b.value) =>
                {
                    Some((b, a))
                }
                _ => None,
            };
            if let Some((string, peer)) = scalar_string {
                self.tick()?;
                let prepared = self
                    .evaluator()
                    .prepare_scalar_string_variant_comparison(
                        &a.value,
                        &b.value,
                        ArithmeticDeclarations {
                            left: a.declaration.ok_or(QueryError::Unsupported)?,
                            right: b.declaration.ok_or(QueryError::Unsupported)?,
                        },
                    )
                    .map_err(predicate_error)?;
                let Some(prepared) = prepared else {
                    return Ok(None);
                };
                let left_owned;
                let right_owned;
                let (left, right) = match prepared {
                    PreparedScalarStringVariantComparison::Original => (&a.value, &b.value),
                    PreparedScalarStringVariantComparison::ConvertedLeft(value) => {
                        left_owned = ExpressionValue::Value(value);
                        (&left_owned, &b.value)
                    }
                    PreparedScalarStringVariantComparison::ConvertedRight(value) => {
                        right_owned = ExpressionValue::Value(value);
                        (&a.value, &right_owned)
                    }
                };
                // Only original actual Strings carry Text comparison authority.
                // Formatting failures precede provider activation; NULL and invalid
                // expression states never activate a provider.
                let string_policy = if expression_string(&peer.value) {
                    policy.merge(string.policy).merge(peer.policy)
                } else {
                    policy.merge(string.policy)
                };
                let collation = if expression_string(left) && expression_string(right) {
                    self.string_comparator(string_policy)?
                } else {
                    None
                };
                let evaluator = self.evaluator();
                let mut evaluator = if let Some(collation) = collation {
                    evaluator.with_collation(collation)
                } else {
                    evaluator
                };
                let result = evaluator
                    .compare_expression_values(op, left, right)
                    .map_err(predicate_error)?;
                self.tick()?;
                return Ok(Some(result));
            }
        }

        Ok(None)
    }
}

fn expression_string(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::StringBytes(_) | ExpressionValue::Value(Value::Text(_))
    )
}

fn ambiguous_string_numeric(left: &CellValue, right: &CellValue) -> bool {
    let (string, numeric) = if expression_string(&left.value) && numeric_string_peer(&right.value) {
        (left, right)
    } else if expression_string(&right.value) && numeric_string_peer(&left.value) {
        (right, left)
    } else {
        return false;
    };
    match (string.declaration, numeric.declaration) {
        (Some(OperandDeclaration::Scalar), None)
        | (None, Some(OperandDeclaration::Variant))
        | (None, None) => true,
        (Some(OperandDeclaration::Variant), None) => !matches!(
            numeric.value,
            ExpressionValue::Value(Value::Boolean(_) | Value::DateTime(_))
        ),
        _ => false,
    }
}

fn numeric_string_peer(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::Value(
            Value::Boolean(_)
                | Value::Byte(_)
                | Value::Int16(_)
                | Value::Int32(_)
                | Value::Int64(_)
                | Value::Single(_)
                | Value::Double(_)
                | Value::Currency(_)
                | Value::Decimal(_)
                | Value::DateTime(_)
        )
    )
}

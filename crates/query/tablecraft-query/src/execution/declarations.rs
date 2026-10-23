//! Operation-local arithmetic declaration provenance, independent of stored values.
pub(super) mod functions;
use super::Runtime;
use crate::{
    QueryError,
    execution::cell::{CellValue, into_operator_scalar},
    source::TextPolicy,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    functions::conversions::Conversion,
    numeric::{Arithmetic, ArithmeticDeclarations, OperandDeclaration},
};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

fn combined(
    left: Option<OperandDeclaration>,
    right: Option<OperandDeclaration>,
) -> Option<OperandDeclaration> {
    match (left, right) {
        (Some(OperandDeclaration::Variant), _) | (_, Some(OperandDeclaration::Variant)) => {
            Some(OperandDeclaration::Variant)
        }
        (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Scalar)) => {
            Some(OperandDeclaration::Scalar)
        }
        _ => None,
    }
}
fn resolved(
    left: Option<OperandDeclaration>,
    right: Option<OperandDeclaration>,
) -> Result<ArithmeticDeclarations, QueryError> {
    if combined(left, right).is_none() {
        return Err(QueryError::ArithmeticDeclaration);
    }
    // A known Variant participant establishes widening independently of the
    // other category. Unknown never establishes that both operands are scalar.
    Ok(ArithmeticDeclarations {
        left: left.unwrap_or(OperandDeclaration::Scalar),
        right: right.unwrap_or(OperandDeclaration::Scalar),
    })
}
fn integral(value: &Value) -> bool {
    matches!(
        value,
        Value::Boolean(_) | Value::Byte(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_)
    )
}
impl Runtime<'_, '_> {
    pub(super) fn declared_binary(
        &mut self,
        operator: B,
        left: CellValue,
        right: CellValue,
    ) -> Result<CellValue, QueryError> {
        let declaration = combined(left.declaration, right.declaration);
        let declarations = (left.declaration, right.declaration);
        let policy = if matches!(operator, B::Add | B::Concatenate) {
            left.policy.merge(right.policy)
        } else {
            TextPolicy::Default
        };
        let left = into_operator_scalar(left)?;
        let right = into_operator_scalar(right)?;
        let operation = match operator {
            B::Add => Some(Arithmetic::Add),
            B::Subtract => Some(Arithmetic::Subtract),
            B::Multiply => Some(Arithmetic::Multiply),
            B::Divide => Some(Arithmetic::Divide),
            B::IntegerDivide => Some(Arithmetic::IntegerDivide),
            B::Modulo => Some(Arithmetic::Modulo),
            B::Power => Some(Arithmetic::Power),
            B::Concatenate => None,
            _ => return Err(QueryError::Unsupported),
        };
        let value = if self.options.declared_integral
            && integral(&left)
            && integral(&right)
            && !matches!(operator, B::Divide | B::Power)
            && let Some(operation) = operation
        {
            let declarations = resolved(declarations.0, declarations.1)?;
            self.evaluator().apply_declared_integral_arithmetic(
                operation,
                &left,
                &right,
                declarations,
            )?
        } else {
            self.evaluator().apply_binary(operator, &left, &right)?
        };
        Ok(CellValue::scalar(value)
            .with_policy(policy)
            .with_declaration(declaration))
    }
    pub(super) fn declared_unary(
        &mut self,
        operator: U,
        operand: CellValue,
    ) -> Result<CellValue, QueryError> {
        let policy = TextPolicy::Default;
        let declaration = operand.declaration;
        let value = into_operator_scalar(operand)?;
        let result = if self.options.declared_integral && operator == U::Minus && integral(&value) {
            let declaration = declaration.ok_or(QueryError::ArithmeticDeclaration)?;
            let value = if matches!(value, Value::Boolean(_) | Value::Byte(_)) {
                Conversion::Integer.apply(&value)?
            } else {
                value
            };
            self.evaluator().apply_declared_integral_arithmetic(
                Arithmetic::Subtract,
                &Value::Int16(0),
                &value,
                ArithmeticDeclarations {
                    left: OperandDeclaration::Scalar,
                    right: declaration,
                },
            )?
        } else {
            self.evaluator().apply_unary(operator, value)?
        };
        Ok(CellValue::scalar(result)
            .with_policy(policy)
            .with_declaration(declaration))
    }
}

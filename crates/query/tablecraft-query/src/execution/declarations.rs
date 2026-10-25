//! Operation-local operator declaration provenance, independent of stored values.
use crate::plan::declarations::combined;
pub(super) use crate::plan::declarations::functions;
mod predicates;
pub(super) mod relational;
use super::Runtime;
use crate::{
    QueryError,
    execution::cell::{CellValue, into_operator_scalar},
    source::TextPolicy,
};
use std::mem::discriminant;
use tablecraft_core::values::Value;
use tablecraft_expressions::evaluation::{
    arithmetic_operands, arithmetic_unary_operand, empty_division_operand,
};
use tablecraft_expressions::numeric::{
    Arithmetic, ArithmeticDeclarations, OperandDeclaration, uses_declared_double_division,
    uses_declared_overflow,
};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

fn resolved(
    left: Option<OperandDeclaration>,
    right: Option<OperandDeclaration>,
) -> Option<ArithmeticDeclarations> {
    combined(left, right)?;
    // A known Variant participant establishes widening independently of the
    // other category. Unknown never establishes that both operands are scalar.
    Some(ArithmeticDeclarations {
        left: left.unwrap_or(OperandDeclaration::Scalar),
        right: right.unwrap_or(OperandDeclaration::Scalar),
    })
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
        if matches!(
            operator,
            B::And | B::Or | B::Xor | B::Equivalence | B::Implication
        ) {
            let value =
                self.evaluator()
                    .apply_expression_binary(operator, left.value, right.value)?;
            return Ok(CellValue::scalar(value).with_declaration(declaration));
        }

        if operator == B::Divide
            && let Some(numerator) = empty_division_operand(&left.value, &right.value)
        {
            let value = self.evaluator().apply_empty_division(numerator)?;
            return Ok(CellValue::scalar(value)
                .with_policy(policy)
                .with_declaration(declaration));
        }
        let operation = match operator {
            B::Add => Some(Arithmetic::Add),
            B::Subtract => Some(Arithmetic::Subtract),
            B::Multiply => Some(Arithmetic::Multiply),
            B::Divide => Some(Arithmetic::Divide),
            B::IntegerDivide => Some(Arithmetic::IntegerDivide),
            B::Modulo => Some(Arithmetic::Modulo),
            B::Power => Some(Arithmetic::Power),
            B::Concatenate | B::And | B::Or | B::Xor | B::Equivalence | B::Implication => None,
            _ => return Err(QueryError::Unsupported),
        };
        let (left, right) = if operation.is_some() {
            arithmetic_operands(operator, left.value, right.value)?
        } else {
            (into_operator_scalar(left)?, into_operator_scalar(right)?)
        };
        let value = if self.options.declared_integral
            && let Some(operation) = operation
            && (uses_declared_overflow(operation, &left, &right)
                || uses_declared_double_division(operation, &left, &right))
        {
            if let Some(declarations) = resolved(declarations.0, declarations.1) {
                if operation == Arithmetic::IntegerDivide {
                    self.evaluator().apply_declared_rounded_arithmetic(
                        operation,
                        &left,
                        &right,
                        declarations,
                    )?
                } else {
                    self.evaluator().apply_declared_numeric_arithmetic(
                        operation,
                        &left,
                        &right,
                        declarations,
                    )?
                }
            } else {
                self.evaluator()
                    .apply_arithmetic_agreement(operation, &left, &right)?
                    .ok_or(QueryError::ArithmeticDeclaration)?
            }
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
        if operator == U::Not {
            let value = self
                .evaluator()
                .apply_expression_complement(operand.value)?;
            return Ok(CellValue::scalar(value).with_declaration(declaration));
        }

        let value = if matches!(operator, U::Plus | U::Minus) {
            arithmetic_unary_operand(operator, operand.value)?
        } else {
            into_operator_scalar(operand)?
        };
        let selected_width = self.options.declared_integral
            && operator == U::Minus
            && matches!(
                value,
                Value::Int16(_) | Value::Int32(_) | Value::DateTime(_)
            );
        let input_carrier = discriminant(&value);
        let result = if selected_width && let Some(declaration) = declaration {
            self.evaluator().apply_declared_numeric_arithmetic(
                Arithmetic::Subtract,
                &Value::Int16(0),
                &value,
                ArithmeticDeclarations {
                    left: OperandDeclaration::Scalar,
                    right: declaration,
                },
            )?
        } else {
            let result = self.evaluator().apply_unary(operator, value)?;
            if selected_width && discriminant(&result) != input_carrier {
                // Only an unknown declaration reaches this branch. Matching
                // carriers establish equal Scalar/Variant outcomes for this
                // operation; widening needs a category we cannot manufacture.
                return Err(QueryError::ArithmeticDeclaration);
            }

            result
        };
        Ok(CellValue::scalar(result)
            .with_policy(policy)
            .with_declaration(declaration))
    }
}

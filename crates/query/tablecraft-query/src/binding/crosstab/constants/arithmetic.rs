//! Selected-width constant arithmetic follows the captured preparation policy.
use super::super::super::Binder;
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::Value;
use tablecraft_expressions::numeric::{
    Arithmetic, ArithmeticDeclarations, OperandDeclaration, uses_declared_overflow,
};
use tablecraft_sql::select::{BinaryOperator as B, UnaryOperator as U};

impl Binder<'_> {
    pub(super) fn constant_unary(
        &mut self,
        operator: U,
        expression: &Expr,
        value: Value,
        depth: usize,
    ) -> Result<Value, QueryError> {
        if self.declared_arithmetic
            && operator == U::Minus
            && matches!(
                value,
                Value::Int16(_) | Value::Int32(_) | Value::DateTime(_)
            )
        {
            let declaration = self
                .result_declaration(expression, &[], depth)?
                .ok_or(QueryError::ArithmeticDeclaration)?;
            return self.heading_operation(0, |evaluator| {
                evaluator.apply_declared_numeric_arithmetic(
                    Arithmetic::Subtract,
                    &Value::Int16(0),
                    &value,
                    ArithmeticDeclarations {
                        left: OperandDeclaration::Scalar,
                        right: declaration,
                    },
                )
            });
        }

        self.heading_operation(0, |evaluator| evaluator.apply_unary(operator, value))
    }

    pub(super) fn constant_binary(
        &mut self,
        operator: B,
        expressions: (&Expr, &Expr),
        values: (&Value, &Value),
        bytes: usize,
        depth: usize,
    ) -> Result<Value, QueryError> {
        let operation = match operator {
            B::Add => Some(Arithmetic::Add),
            B::Subtract => Some(Arithmetic::Subtract),
            B::Multiply => Some(Arithmetic::Multiply),
            B::Divide => Some(Arithmetic::Divide),
            B::IntegerDivide => Some(Arithmetic::IntegerDivide),
            B::Modulo => Some(Arithmetic::Modulo),
            B::Power => Some(Arithmetic::Power),
            _ => None,
        };
        let (left, right) = values;
        if self.declared_arithmetic
            && let Some(operation) = operation
            && uses_declared_overflow(operation, left, right)
        {
            // Closed admitted constants have explicit Scalar/Variant provenance.
            // Missing provenance must not manufacture a metadata-only heading.
            let declarations = ArithmeticDeclarations {
                left: self
                    .result_declaration(expressions.0, &[], depth)?
                    .ok_or(QueryError::ArithmeticDeclaration)?,
                right: self
                    .result_declaration(expressions.1, &[], depth)?
                    .ok_or(QueryError::ArithmeticDeclaration)?,
            };
            return self.heading_operation(bytes, |evaluator| {
                if operation == Arithmetic::IntegerDivide {
                    evaluator.apply_declared_rounded_arithmetic(
                        operation,
                        left,
                        right,
                        declarations,
                    )
                } else {
                    evaluator.apply_declared_numeric_arithmetic(
                        operation,
                        left,
                        right,
                        declarations,
                    )
                }
            });
        }

        self.heading_operation(bytes, |evaluator| {
            evaluator.apply_binary(operator, left, right)
        })
    }
}

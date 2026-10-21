//! Explicit array intermediates are never published as scalar query cells.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    arrays::{ArrayValue, ArrayView, BoundFunction},
    functions::inspection::types::{InspectionInput, ValueInspectionFunction},
};

pub(super) enum Operand {
    Scalar(Value),
    Array(ArrayValue),
}

impl Operand {
    fn inspection(&self) -> InspectionInput<'_> {
        match self {
            Self::Scalar(value) => InspectionInput::Scalar(value),
            Self::Array(array) => InspectionInput::Array(ArrayView::Variants(array)),
        }
    }

    fn array(&self) -> Result<ArrayView<'_>, EvalError> {
        match self {
            Self::Array(array) => Ok(ArrayView::Variants(array)),
            Self::Scalar(Value::Binary(bytes)) => Ok(ArrayView::Bytes(bytes)),
            Self::Scalar(Value::Null) => Err(EvalError::InvalidNullConversion),
            Self::Scalar(_) => Err(EvalError::TypeMismatch),
        }
    }
}

impl Runtime<'_> {
    fn inspection_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<Operand, QueryError> {
        if let Expr::Array(elements) = expression {
            self.tick()?;
            self.budget
                .charge_allocation(elements.len() as u64, size_of::<Value>() as u64)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| QueryError::Allocation)?;

            for element in elements {
                values.push(self.eval(element, env)?);
            }

            return Ok(Operand::Array(ArrayValue::from_owned(values, 0)?));
        }

        self.eval(expression, env).map(Operand::Scalar)
    }

    pub(super) fn value_inspection(
        &mut self,
        function: ValueInspectionFunction,
        input: &Expr,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let input = self.inspection_operand(input, env)?;
        Ok(self
            .evaluator()
            .apply_value_inspection(function, input.inspection())?)
    }

    pub(super) fn array_bound(
        &mut self,
        function: BoundFunction,
        input: &Expr,
        dimension: &Expr,
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let input = self.inspection_operand(input, env)?;
        let dimension = self.eval(dimension, env)?;

        Ok(self
            .evaluator()
            .apply_array_bound(function, input.array()?, Some(&dimension))?)
    }
}

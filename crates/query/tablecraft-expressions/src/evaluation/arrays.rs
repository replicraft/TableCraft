//! Array intermediates are consumed explicitly and never coerced to stored scalars.
use super::Evaluator;
use crate::{
    EvalError,
    arrays::{ArrayValue, ArrayView, BoundFunction, sql_arguments},
    functions::{
        Builtin,
        inspection::types::InspectionInput,
        text::byte_string::{ByteOperand, ByteString},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_sql::select::{BinaryOperator, Expression, FunctionArgument};

pub(super) enum InspectionOperand<'a> {
    Scalar(Value),
    Expression(ExpressionValue),
    ByteString(ByteString),
    Array(ArrayValue),
    Binary(&'a [u8]),
}

impl InspectionOperand<'_> {
    pub(super) fn inspection(&self) -> InspectionInput<'_> {
        match self {
            Self::Scalar(value) => InspectionInput::Scalar(value),
            Self::Expression(value) => InspectionInput::Expression(value),
            Self::ByteString(bytes) => InspectionInput::ByteString(bytes),
            Self::Binary(bytes) => InspectionInput::Array(ArrayView::Bytes(bytes)),
            Self::Array(value) => InspectionInput::Array(ArrayView::Variants(value)),
        }
    }

    pub(super) fn array(&self) -> Result<ArrayView<'_>, EvalError> {
        match self {
            Self::Array(value) => Ok(ArrayView::Variants(value)),
            Self::Scalar(Value::Binary(bytes)) => Ok(ArrayView::Bytes(bytes)),
            Self::Expression(ExpressionValue::Value(Value::Binary(bytes))) => {
                Ok(ArrayView::Bytes(bytes))
            }
            Self::Expression(ExpressionValue::Value(Value::Null)) => {
                Err(EvalError::InvalidNullConversion)
            }
            Self::Binary(bytes) => Ok(ArrayView::Bytes(bytes)),
            Self::Scalar(Value::Null) => Err(EvalError::InvalidNullConversion),
            Self::Scalar(_) | Self::ByteString(_) | Self::Expression(_) => {
                Err(EvalError::TypeMismatch)
            }
        }
    }
}

impl<'a> Evaluator<'a> {
    pub(super) fn inspection_operand(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<InspectionOperand<'a>, EvalError> {
        if matches!(
            expression,
            Expression::Binary {
                operator: BinaryOperator::Concatenate | BinaryOperator::Add,
                ..
            }
        ) {
            return Ok(match self.byte_operand(expression, depth)? {
                ByteOperand::Scalar(value) => InspectionOperand::Scalar(value),
                ByteOperand::Bytes(bytes) => InspectionOperand::ByteString(bytes),
            });
        }
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && sql_arguments::is_constructor(qualifier, name)
        {
            self.check(depth)?;
            let elements =
                sql_arguments::elements(arguments).ok_or(EvalError::UnsupportedExpression)?;
            self.budget
                .charge_allocation(elements.len() as u64, size_of::<Value>() as u64)?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| EvalError::Allocation)?;

            for argument in elements {
                let FunctionArgument::Expression(element) = argument else {
                    return Err(EvalError::UnsupportedExpression);
                };
                values.push(self.node(element, depth + 1)?);
            }

            return ArrayValue::from_owned(values, 0).map(InspectionOperand::Array);
        }

        if let Expression::Call {
            qualifier, name, ..
        } = expression
            && qualifier.is_empty()
        {
            let builtin = Builtin::resolve(name.as_str());
            if builtin == Some(Builtin::CVar)
                && let Expression::Call { arguments, .. } = expression
                && let [FunctionArgument::Expression(value)] = arguments.as_slice()
            {
                self.check(depth)?;
                let input = self.inspection_operand(value, depth + 1)?;
                if matches!(
                    input,
                    InspectionOperand::Array(_) | InspectionOperand::Binary(_)
                ) {
                    return Err(EvalError::UnsupportedExpression);
                }
                return Ok(input);
            }
            if matches!(
                builtin,
                Some(
                    Builtin::Eval
                        | Builtin::CvErr
                        | Builtin::IIf
                        | Builtin::Nz
                        | Builtin::Conditional(_)
                )
            ) {
                return self
                    .expression_operand(expression, depth)
                    .map(InspectionOperand::Expression);
            }

            if matches!(builtin, Some(Builtin::Text(function)) if function.produces_bytes()) {
                return Ok(match self.byte_operand(expression, depth)? {
                    ByteOperand::Scalar(value) => InspectionOperand::Scalar(value),
                    ByteOperand::Bytes(bytes) => InspectionOperand::ByteString(bytes),
                });
            }
        }

        if let Expression::Field(field) = expression {
            self.check(depth)?;
            let bindings = self.bindings;
            let value = bindings.resolve(field)?;
            if let Value::Binary(bytes) = value {
                return Ok(InspectionOperand::Binary(bytes));
            }

            return self.binding_value(value).map(InspectionOperand::Scalar);
        }

        self.node(expression, depth).map(InspectionOperand::Scalar)
    }

    /// Query a dimension under this evaluator's cumulative work/cancellation budget.
    pub fn apply_array_bound(
        &mut self,
        function: BoundFunction,
        array: ArrayView<'_>,
        dimension: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        function.apply(array, dimension)
    }

    pub(super) fn array_bound(
        &mut self,
        function: BoundFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<Value, EvalError> {
        let [FunctionArgument::Expression(input), options @ ..] = arguments else {
            return Err(EvalError::UnsupportedExpression);
        };
        if options.len() > 1
            || options.iter().any(|arg| {
                !matches!(
                    arg,
                    FunctionArgument::Expression(_) | FunctionArgument::Missing
                )
            })
        {
            return Err(EvalError::UnsupportedExpression);
        }

        let input = self.inspection_operand(input, depth + 1)?;
        let dimension = match options.first() {
            Some(FunctionArgument::Expression(expression)) => {
                Some(self.node(expression, depth + 1)?)
            }
            _ => None,
        };

        self.apply_array_bound(function, input.array()?, dimension.as_ref())
    }
}

//! Closed policy-free String construction keeps raw units until the final name.
use super::super::super::Binder;
use crate::{QueryError, plan::Expr};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::{
    EvalError,
    evaluation::PreparedAddition,
    functions::{
        conditional::expression,
        text::{TextFunction, byte_string::ExpressionByteOperand},
    },
};
use tablecraft_sql::select::BinaryOperator;

const UTF16_UNIT_BYTES: usize = size_of::<u16>();
const MAX_UTF8_BYTES_PER_UNIT: usize = 3;

impl Binder<'_> {
    pub(super) fn constant_text_heading(
        &mut self,
        input: &Expr,
        depth: usize,
    ) -> Result<Value, QueryError> {
        match self.constant_string_operand(input, depth)? {
            ExpressionValue::Value(value) => Ok(value),
            ExpressionValue::StringBytes(bytes) => {
                if !bytes.len().is_multiple_of(UTF16_UNIT_BYTES) {
                    return Err(QueryError::Unsupported);
                }
                let allocation = (bytes.len() / UTF16_UNIT_BYTES)
                    .checked_mul(MAX_UTF8_BYTES_PER_UNIT)
                    .ok_or(QueryError::StructureLimit)?;
                match self.heading_operation(allocation, |evaluator| {
                    evaluator.publish_expression_string(&bytes)
                }) {
                    // A raw intermediate need not have a publishable name. Do
                    // not repair it or turn metadata inability into new syntax.
                    Err(QueryError::Evaluation(EvalError::UnrepresentableText)) => {
                        Err(QueryError::Unsupported)
                    }
                    result => result,
                }
            }
            _ => Err(QueryError::Unsupported),
        }
    }

    fn constant_string_operand(
        &mut self,
        input: &Expr,
        depth: usize,
    ) -> Result<ExpressionValue, QueryError> {
        self.tick(depth)?;
        match input {
            Expr::TextCall(function, arguments) => {
                self.constant_text_call(*function, arguments, depth + 1)
            }
            Expr::CVar(value) => self.constant_string_operand(value, depth + 1),
            Expr::IIf(condition, yes, no) => {
                let condition = self.constant_string_operand(condition, depth + 1)?;
                let yes = self.constant_string_operand(yes, depth + 1)?;
                let no = self.constant_string_operand(no, depth + 1)?;
                if !selector_control(&condition) {
                    return Err(QueryError::Unsupported);
                }
                expression::iif(&condition, yes, no).map_err(QueryError::Evaluation)
            }
            Expr::Nz(value, replacement) => {
                let value = self.constant_string_operand(value, depth + 1)?;
                let replacement = replacement
                    .as_deref()
                    .map(|value| self.constant_string_operand(value, depth + 1))
                    .transpose()?;
                Ok(expression::nz(value, replacement))
            }
            Expr::ConditionalCall(function, arguments) => {
                let mut selection = function
                    .start(arguments.len())
                    .map_err(QueryError::Evaluation)?;
                let mut contextual_control = false;
                for argument in arguments {
                    let value = self.constant_string_operand(argument, depth + 1)?;
                    contextual_control |= selection.is_control() && !selector_control(&value);
                    selection
                        .push_expression(value)
                        .map_err(QueryError::Evaluation)?;
                }
                if contextual_control {
                    return Err(QueryError::Unsupported);
                }
                selection
                    .finish_expression()
                    .map_err(QueryError::Evaluation)
            }
            Expr::Binary(
                operator @ (BinaryOperator::Add | BinaryOperator::Concatenate),
                left,
                right,
            ) => self.constant_string_binary(*operator, left, right, depth + 1),
            _ => self
                .constant_heading(input, depth + 1)
                .map(ExpressionValue::Value),
        }
    }

    fn constant_text_call(
        &mut self,
        function: TextFunction,
        arguments: &[Expr],
        depth: usize,
    ) -> Result<ExpressionValue, QueryError> {
        if !admitted(function)
            || !function.accepts_arity(arguments.len())
            || arguments.len() > TextFunction::MAX_ARGUMENTS
        {
            return Err(QueryError::Unsupported);
        }
        let mut values: [ExpressionValue; TextFunction::MAX_ARGUMENTS] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        for (argument, value) in arguments.iter().zip(&mut values) {
            *value = self.constant_string_operand(argument, depth + 1)?;
        }
        let constructor = matches!(function, TextFunction::ChrB | TextFunction::ChrW);
        if constructor {
            if !numeric_control(&values[0]) {
                return Err(QueryError::Unsupported);
            }
        } else if !string_source(&values[0]) {
            // Implicit scalar CStr would require an execution-local policy.
            return Err(QueryError::Unsupported);
        }
        if values[1..arguments.len()]
            .iter()
            .any(|value| !numeric_control(value))
        {
            return Err(QueryError::Unsupported);
        }

        let bytes = if constructor {
            match function {
                TextFunction::ChrW => UTF16_UNIT_BYTES,
                TextFunction::ChrB => size_of::<u8>(),
                _ => return Err(QueryError::Unsupported),
            }
        } else if function.observes_bytes() {
            0
        } else {
            let extent = self.constant_string_extent(&values[0])?;
            match function {
                TextFunction::StrReverse => {
                    // Reserve source encoding and the reversed output buffer.
                    extent
                        .checked_add(extent)
                        .ok_or(QueryError::StructureLimit)?
                }
                TextFunction::LTrim | TextFunction::RTrim | TextFunction::Trim => {
                    match &values[0] {
                        ExpressionValue::Value(Value::Text(text)) => text.len(),
                        _ => 0,
                    }
                }
                _ => match values[0] {
                    ExpressionValue::Value(Value::Text(_)) => extent,
                    _ => 0,
                },
            }
        };
        let first = std::mem::replace(&mut values[0], ExpressionValue::Value(Value::Null));
        self.heading_operation(bytes, |evaluator| {
            let first = ExpressionByteOperand::try_from(first)?;
            if function.observes_bytes() {
                evaluator
                    .observe_expression_byte_operand(function, first)
                    .map(ExpressionValue::Value)
            } else {
                evaluator
                    .apply_expression_byte_string(function, first, &values[1..arguments.len()])
                    .map(ExpressionByteOperand::from)
                    .map(ExpressionByteOperand::into_expression_value)
            }
        })
    }

    fn constant_string_binary(
        &mut self,
        operator: BinaryOperator,
        left: &Expr,
        right: &Expr,
        depth: usize,
    ) -> Result<ExpressionValue, QueryError> {
        let a = self.constant_string_operand(left, depth + 1)?;
        let b = self.constant_string_operand(right, depth + 1)?;
        if !string_source(&a) || !string_source(&b) {
            if operator == BinaryOperator::Add
                && let (ExpressionValue::Value(a), ExpressionValue::Value(b)) = (a, b)
                && !matches!(a, Value::Text(_))
                && !matches!(b, Value::Text(_))
            {
                return self
                    .constant_binary(operator, (left, right), (&a, &b), 0, depth)
                    .map(ExpressionValue::Value);
            }
            return Err(QueryError::Unsupported);
        }
        if let (ExpressionValue::Value(a), ExpressionValue::Value(b)) = (&a, &b) {
            let length = |value: &Value| match value {
                Value::Text(text) => text.len(),
                _ => 0,
            };
            let bytes = length(a)
                .checked_add(length(b))
                .ok_or(QueryError::StructureLimit)?;
            return self
                .constant_binary(operator, (left, right), (a, b), bytes, depth)
                .map(ExpressionValue::Value);
        }

        let a_bytes = self.constant_string_extent(&a)?;
        let b_bytes = self.constant_string_extent(&b)?;
        let bytes = a_bytes
            .checked_add(b_bytes)
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or(QueryError::StructureLimit)?;
        self.heading_operation(bytes, |evaluator| {
            let a = ExpressionByteOperand::try_from(a)?;
            let b = ExpressionByteOperand::try_from(b)?;
            let value = if operator == BinaryOperator::Add {
                match evaluator.prepare_expression_addition(a, b)? {
                    PreparedAddition::StringResult(value) => value,
                    PreparedAddition::ScalarOperands { .. } => {
                        return Err(EvalError::UnsupportedExpression);
                    }
                }
            } else {
                evaluator.apply_expression_concatenation(a, b)?
            };
            Ok(ExpressionByteOperand::from(value).into_expression_value())
        })
    }

    fn constant_string_extent(&mut self, value: &ExpressionValue) -> Result<usize, QueryError> {
        self.tick(1)?;
        match value {
            ExpressionValue::Value(Value::Text(text)) => {
                if let Some(budget) = &mut self.budget {
                    budget.charge(Resource::Steps, text.len() as u64)?;
                }
                text.encode_utf16()
                    .count()
                    .checked_mul(UTF16_UNIT_BYTES)
                    .ok_or(QueryError::StructureLimit)
            }
            ExpressionValue::StringBytes(bytes) => Ok(bytes.len()),
            ExpressionValue::Value(Value::Null) => Ok(0),
            _ => Err(QueryError::Unsupported),
        }
    }
}

fn string_source(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::Value(Value::Text(_) | Value::Null) | ExpressionValue::StringBytes(_)
    )
}

fn selector_control(value: &ExpressionValue) -> bool {
    // Existing closed choices admit ordinary non-Text controls and let the
    // shared conversion report invalid carriers. Retained Strings remain
    // contextual; do not publish them just to test eligibility.
    matches!(value, ExpressionValue::Value(value) if !matches!(value, Value::Text(_)))
}

fn numeric_control(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::Value(
            Value::Null
                | Value::Boolean(_)
                | Value::Byte(_)
                | Value::Int16(_)
                | Value::Int32(_)
                | Value::Int64(_)
                | Value::Single(_)
                | Value::Double(_)
                | Value::Currency(_)
                | Value::Decimal(_)
        )
    )
}

fn admitted(function: TextFunction) -> bool {
    matches!(
        function,
        TextFunction::Len
            | TextFunction::LenB
            | TextFunction::AscW
            | TextFunction::AscB
            | TextFunction::Left
            | TextFunction::Right
            | TextFunction::Mid
            | TextFunction::LeftB
            | TextFunction::RightB
            | TextFunction::MidB
            | TextFunction::LTrim
            | TextFunction::RTrim
            | TextFunction::Trim
            | TextFunction::StrReverse
            | TextFunction::ChrW
            | TextFunction::ChrB
    )
}

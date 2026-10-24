//! Owning byte-string evaluation and strict scalar publication.
mod reversal;
mod trimming;
mod unicode;

use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        Builtin,
        conversions::Conversion,
        text::{
            TextFunction,
            byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
            characters,
            sql_arguments::{self, Argument},
        },
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_sql::select::{BinaryOperator, Expression, FunctionArgument};

const UTF16_UNIT_BYTES: usize = size_of::<u16>();

impl Evaluator<'_> {
    pub(super) fn expression_byte_operand(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<ExpressionByteOperand, EvalError> {
        if let Expression::Binary {
            operator: BinaryOperator::Add,
            left,
            right,
        } = expression
        {
            self.check(depth)?;
            return self.addition_call(left, right, depth).map(Into::into);
        }
        if let Expression::Binary {
            operator: BinaryOperator::Concatenate,
            left,
            right,
        } = expression
        {
            self.check(depth)?;
            return self.concatenation_call(left, right, depth).map(Into::into);
        }
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && qualifier.is_empty()
        {
            match Builtin::resolve(name.as_str()) {
                Some(Builtin::Nz) => {
                    return self.nz_byte_operand(arguments, depth);
                }
                Some(Builtin::Conditional(function)) => {
                    return self.selection_byte_operand(function, arguments, depth);
                }
                Some(Builtin::IIf) => {
                    if let [
                        FunctionArgument::Expression(condition),
                        FunctionArgument::Expression(yes),
                        FunctionArgument::Expression(no),
                    ] = arguments.as_slice()
                    {
                        self.check(depth)?;
                        return self.iif_byte_operand(condition, yes, no, depth);
                    }
                }
                Some(Builtin::Text(function)) if function.produces_bytes() => {
                    return self.byte_operand(expression, depth).map(Into::into);
                }
                Some(Builtin::CVar) => {
                    if let [FunctionArgument::Expression(value)] = arguments.as_slice() {
                        self.check(depth)?;
                        return self.expression_byte_operand(value, depth + 1);
                    }
                }
                _ => {}
            }
        }
        self.expression_operand(expression, depth)
            .and_then(ExpressionByteOperand::try_from)
    }

    pub(super) fn iif_byte_operand(
        &mut self,
        condition: &Expression,
        yes: &Expression,
        no: &Expression,
        depth: usize,
    ) -> Result<ExpressionByteOperand, EvalError> {
        let condition = self.expression_operand(condition, depth + 1)?;
        let yes = self.expression_byte_operand(yes, depth + 1)?;
        let no = self.expression_byte_operand(no, depth + 1)?;

        self.apply_iif(&condition, yes, no)
    }

    pub(super) fn publish_expression_byte_value(
        &mut self,
        value: ExpressionByteOperand,
    ) -> Result<ExpressionValue, EvalError> {
        match value {
            ExpressionByteOperand::Scalar(value) => Ok(value),
            ExpressionByteOperand::Bytes(bytes) => self
                .publish_byte_operand(ByteOperand::Bytes(bytes))
                .map(ExpressionValue::Value),
        }
    }

    pub(super) fn byte_operand(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        if let Expression::Binary {
            operator: BinaryOperator::Add,
            left,
            right,
        } = expression
        {
            self.check(depth)?;
            return self.addition_call(left, right, depth);
        }
        if let Expression::Binary {
            operator: BinaryOperator::Concatenate,
            left,
            right,
        } = expression
        {
            self.check(depth)?;
            return self.concatenation_call(left, right, depth);
        }
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && qualifier.is_empty()
        {
            match Builtin::resolve(name.as_str()) {
                Some(Builtin::Text(function)) if function.produces_bytes() => {
                    self.check(depth)?;
                    return self.byte_call(function, arguments, depth);
                }
                Some(Builtin::CVar) => {
                    if let [FunctionArgument::Expression(value)] = arguments.as_slice() {
                        self.check(depth)?;
                        return self.byte_operand(value, depth + 1);
                    }
                }
                _ => {}
            }
        }
        self.node(expression, depth).map(ByteOperand::Scalar)
    }

    pub(super) fn byte_call(
        &mut self,
        function: TextFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        if function == TextFunction::Replace {
            return self.replacement_byte_call(arguments, depth);
        }
        if function == TextFunction::String {
            return self.string_byte_call(arguments, depth);
        }
        let (first, rest, count) = self.byte_call_inputs(function, arguments, depth)?;
        self.apply_expression_byte_string(function, first, &rest[..count])
    }

    pub(super) fn slice_call(
        &mut self,
        function: TextFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<Value, EvalError> {
        let (first, rest, count) = self.byte_call_inputs(function, arguments, depth)?;
        self.apply_expression_slice_source(function, first, &rest[..count])
    }

    fn byte_call_inputs(
        &mut self,
        function: TextFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<(ExpressionByteOperand, [ExpressionValue; 2], usize), EvalError> {
        let mut arguments = sql_arguments::normalize(function, arguments)
            .ok_or(EvalError::UnsupportedExpression)?;
        let Some(Argument::Expression(first)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let first = self.expression_byte_operand(first, depth + 1)?;
        let mut rest: [ExpressionValue; 2] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        let count = arguments.len();
        for (argument, value) in arguments.zip(&mut rest) {
            *value = match argument {
                Argument::Expression(expression) => {
                    self.expression_operand(expression, depth + 1)?
                }
                Argument::Integer(value) => ExpressionValue::Value(Value::Int64(value)),
                Argument::EmptyText => ExpressionValue::Value(Value::Text(String::new())),
            };
        }
        Ok((first, rest, count))
    }

    /// Admit eager byte-producer states at their original numeric/source stages.
    pub fn apply_expression_byte_string(
        &mut self,
        function: TextFunction,
        first: ExpressionByteOperand,
        rest: &[ExpressionValue],
    ) -> Result<ByteOperand, EvalError> {
        if !function.produces_bytes() || !function.accepts_arity(rest.len() + 1) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(rest)?;
        self.check(0)?;
        if function == TextFunction::CStr {
            return self.apply_expression_cstr_operand(first);
        }
        if function == TextFunction::String {
            return self.apply_retained_string(&first.into_expression_value(), &rest[0]);
        }
        if function.trims_spaces() {
            return self.apply_trim_source(function, first.into_string()?);
        }
        if function == TextFunction::StrReverse {
            return self.apply_reverse_source(first.into_string()?);
        }
        if function.slices_units() {
            return self.apply_unicode_slice(function, first, rest);
        }
        if function == TextFunction::StrConv {
            return self.apply_expression_strconv(first, &rest[0], rest.get(1));
        }
        if matches!(function, TextFunction::ChrB | TextFunction::ChrW) {
            return self.apply_byte_string(function, ByteOperand::Scalar(first.into_code()?), &[]);
        }

        let empty_number = Value::Int32(0);
        self.byte_slice_inputs(
            function,
            || first.into_string(),
            rest.len(),
            |index| text_arguments::scalar(&rest[index], &empty_number),
        )
    }

    /// Apply a byte producer to already evaluated operands in written order.
    /// Retains raw string bytes without converting them to stored Binary.
    pub fn apply_byte_string(
        &mut self,
        function: TextFunction,
        first: ByteOperand,
        rest: &[Value],
    ) -> Result<ByteOperand, EvalError> {
        self.check(0)?;
        if !function.produces_bytes() || !function.accepts_arity(rest.len() + 1) {
            return Err(EvalError::UnsupportedExpression);
        }
        if function == TextFunction::CStr {
            return self.apply_expression_cstr_operand(first.into());
        }
        if function == TextFunction::String {
            let count = ExpressionByteOperand::from(first).into_expression_value();
            return self.repeat_scalar_character(&count, &rest[0]);
        }
        if function.trims_spaces() {
            return self.apply_trim_source(function, first);
        }
        if function == TextFunction::StrReverse {
            return self.apply_reverse_source(first);
        }
        if function.slices_units() {
            return self.unicode_slice(function, first, rest);
        }
        if function == TextFunction::StrConv {
            return self.apply_strconv(first, &rest[0], rest.get(1));
        }
        if matches!(function, TextFunction::ChrB | TextFunction::ChrW) {
            let ByteOperand::Scalar(value) = first else {
                return Err(EvalError::TypeMismatch);
            };
            let code = self.byte_long(&value)?;
            let (unit, length) = if function == TextFunction::ChrW {
                (characters::unicode_unit(code)?, UTF16_UNIT_BYTES)
            } else {
                (
                    u16::from(u8::try_from(code).map_err(|_| EvalError::Overflow)?),
                    size_of::<u8>(),
                )
            };
            self.budget.charge(Resource::Bytes, length as u64)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| EvalError::Allocation)?;
            bytes.extend_from_slice(&unit.to_le_bytes()[..length]);
            return ByteString::from_owned(bytes).map(ByteOperand::Bytes);
        }

        self.byte_slice_inputs(function, || Ok(first), rest.len(), |index| Ok(&rest[index]))
    }

    fn byte_slice_inputs<'a>(
        &mut self,
        function: TextFunction,
        source: impl FnOnce() -> Result<ByteOperand, EvalError>,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<ByteOperand, EvalError> {
        let position = self.byte_long(argument(0)?)?;
        let length = (count > 1)
            .then(|| self.byte_long(argument(1)?))
            .transpose()?;
        if position < 0
            || (function == TextFunction::MidB && position == 0)
            || length.is_some_and(|length| length < 0)
        {
            return Err(EvalError::Domain);
        }
        let first = self.coerce_byte_string_source(source()?)?;
        let bytes = match first {
            ByteOperand::Scalar(Value::Null) => return Ok(ByteOperand::Scalar(Value::Null)),
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text)?,
            ByteOperand::Scalar(_) => return Err(EvalError::TypeMismatch),
            ByteOperand::Bytes(bytes) => bytes,
        };
        let bytes = match function {
            TextFunction::LeftB => bytes.left(position)?,
            TextFunction::RightB => bytes.right(position)?,
            TextFunction::MidB => bytes.mid(position, length)?,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        Ok(ByteOperand::Bytes(bytes))
    }

    /// Convert scalar string arguments under explicit CStr policies. Preserve
    /// raw strings and Binary's distinct array identity for each caller to admit.
    pub(super) fn coerce_byte_string_source(
        &mut self,
        operand: ByteOperand,
    ) -> Result<ByteOperand, EvalError> {
        match operand {
            ByteOperand::Scalar(value)
                if !matches!(value, Value::Text(_) | Value::Null | Value::Binary(_)) =>
            {
                self.apply_text_function(TextFunction::CStr, &[value])
                    .map(ByteOperand::Scalar)
            }
            operand => Ok(operand),
        }
    }

    pub(super) fn encode_byte_string(&mut self, text: &str) -> Result<ByteString, EvalError> {
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut length = 0usize;
        for character in text.chars() {
            self.check(0)?;
            length = length
                .checked_add(character.len_utf16() * size_of::<u16>())
                .ok_or(EvalError::Overflow)?;
        }
        i32::try_from(length).map_err(|_| EvalError::Overflow)?;
        self.budget.charge(Resource::Bytes, length as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| EvalError::Allocation)?;
        for unit in text.encode_utf16() {
            self.check(0)?;
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        ByteString::from_owned(bytes)
    }

    /// Publish only ordinary scalars or strictly representable Unicode strings.
    /// Invalid UTF-16 is never replaced, reinterpreted as Binary or truncated.
    pub fn publish_byte_operand(&mut self, operand: ByteOperand) -> Result<Value, EvalError> {
        match operand {
            ByteOperand::Scalar(value) => Ok(value),
            ByteOperand::Bytes(bytes) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                bytes.write_utf8(&mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }

    /// Observe an intermediate without requiring it to be valid Unicode.
    pub fn observe_byte_operand(
        &mut self,
        function: TextFunction,
        operand: ByteOperand,
    ) -> Result<Value, EvalError> {
        match operand {
            ByteOperand::Scalar(value) => self.apply_text_function(function, &[value]),
            ByteOperand::Bytes(bytes) => {
                self.check(0)?;
                match function {
                    TextFunction::Len | TextFunction::AscW => {
                        let (units, remainder) = bytes.as_bytes().as_chunks::<UTF16_UNIT_BYTES>();
                        if !remainder.is_empty() {
                            return Err(EvalError::UnrepresentableText);
                        }
                        if function == TextFunction::Len {
                            Ok(Value::Int32(bytes.len() / UTF16_UNIT_BYTES as i32))
                        } else {
                            units
                                .first()
                                .copied()
                                .map(i16::from_le_bytes)
                                .map(Value::Int16)
                                .ok_or(EvalError::Domain)
                        }
                    }
                    TextFunction::LenB => Ok(Value::Int32(bytes.len())),
                    TextFunction::AscB => bytes
                        .as_bytes()
                        .first()
                        .copied()
                        .map(|byte| Value::Int16(i16::from(byte)))
                        .ok_or(EvalError::Domain),
                    _ => Err(EvalError::UnsupportedExpression),
                }
            }
        }
    }

    /// Observe scalar states or raw string bytes without Unicode publication.
    pub fn observe_expression_byte_operand(
        &mut self,
        function: TextFunction,
        operand: ExpressionByteOperand,
    ) -> Result<Value, EvalError> {
        if !function.observes_bytes() {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(0)?;
        self.observe_byte_operand(function, operand.into_string()?)
    }
}

impl Evaluator<'_> {
    pub(super) fn byte_long(&mut self, value: &Value) -> Result<i32, EvalError> {
        if matches!(value, Value::Null) {
            return Err(EvalError::InvalidNullConversion);
        }
        match self.apply_conversion(Conversion::Long, value)? {
            Value::Int32(value) => Ok(value),
            _ => Err(EvalError::TypeMismatch),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_sources;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod unicode_tests;

//! Consuming UTF-16 slices retain complete units without Unicode publication.
use super::{ByteOperand, ByteString, EvalError, Evaluator, ExpressionByteOperand, TextFunction};
use crate::evaluation::text_arguments;
use tablecraft_core::values::{ExpressionValue, Value};

const UNIT_BYTES: i32 = size_of::<u16>() as i32;

impl Evaluator<'_> {
    pub(super) fn apply_unicode_slice(
        &mut self,
        function: TextFunction,
        first: ExpressionByteOperand,
        rest: &[ExpressionValue],
    ) -> Result<ByteOperand, EvalError> {
        let empty = Value::Int32(0);
        self.unicode_slice_inputs(function, first.into_string(), rest.len(), |index| {
            text_arguments::scalar(&rest[index], &empty)
        })
    }

    pub(super) fn unicode_slice(
        &mut self,
        function: TextFunction,
        first: ByteOperand,
        rest: &[Value],
    ) -> Result<ByteOperand, EvalError> {
        self.unicode_slice_inputs(function, Ok(first), rest.len(), |index| Ok(&rest[index]))
    }

    fn unicode_slice_inputs<'a>(
        &mut self,
        function: TextFunction,
        first: Result<ByteOperand, EvalError>,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<ByteOperand, EvalError> {
        let first = first?;
        if matches!(first, ByteOperand::Scalar(Value::Null)) {
            return Ok(first);
        }

        let first = self.coerce_byte_string_source(first)?;
        if matches!(first, ByteOperand::Scalar(ref value) if !matches!(value, Value::Text(_))) {
            return Err(EvalError::TypeMismatch);
        }

        let position = self.slice_position(argument(0)?)?;
        if position < 0 || (function == TextFunction::Mid && position == 0) {
            return Err(EvalError::Domain);
        }

        let length = (count > 1)
            .then(|| self.slice_position(argument(1)?))
            .transpose()?;
        if length.is_some_and(|length| length < 0) {
            return Err(EvalError::Domain);
        }

        let bytes = match first {
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text)?,
            ByteOperand::Bytes(bytes) => bytes,
            _ => return Err(EvalError::TypeMismatch),
        };
        slice(function, bytes, position, length).map(ByteOperand::Bytes)
    }
}

fn slice(
    function: TextFunction,
    bytes: ByteString,
    position: i32,
    length: Option<i32>,
) -> Result<ByteString, EvalError> {
    if bytes.len() % UNIT_BYTES != 0 {
        return Err(EvalError::UnrepresentableText);
    }

    let total = bytes.len() / UNIT_BYTES;
    match function {
        TextFunction::Left => bytes.left(position.min(total) * UNIT_BYTES),
        TextFunction::Right => bytes.right(position.min(total) * UNIT_BYTES),
        TextFunction::Mid => {
            let start = (position - 1).min(total);
            let length = length.map(|length| length.min(total - start) * UNIT_BYTES);
            // Clamp in units before multiplying, so maximum Long requests stay
            // within the already validated byte-string length.
            bytes.mid(start * UNIT_BYTES + 1, length)
        }
        _ => Err(EvalError::UnsupportedExpression),
    }
}

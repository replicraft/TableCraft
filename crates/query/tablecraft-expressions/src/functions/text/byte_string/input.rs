//! Owned byte-string input can retain scalar states until its consumer admits them.
use super::{ByteOperand, ByteString};
#[cfg(feature = "sql")]
use crate::EvalError;
use tablecraft_core::values::ExpressionValue;
#[cfg(feature = "sql")]
use tablecraft_core::values::Value;

/// Eager scalar expression data or an already validated raw string intermediate.
/// This input does not make Error/Empty/Missing valid stored values or raw bytes.
#[derive(Debug)]
pub enum ExpressionByteOperand {
    /// Retain scalar states until the owning function selects a conversion target.
    Scalar(ExpressionValue),
    /// Preserve raw bytes, including odd lengths and unpaired UTF-16 units.
    Bytes(ByteString),
}

impl From<ByteOperand> for ExpressionByteOperand {
    fn from(value: ByteOperand) -> Self {
        match value {
            ByteOperand::Scalar(value) => Self::Scalar(ExpressionValue::Value(value)),
            ByteOperand::Bytes(bytes) => Self::Bytes(bytes),
        }
    }
}

#[cfg(feature = "sql")]
impl ExpressionByteOperand {
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Self::Scalar(ExpressionValue::Value(Value::Null)))
    }

    pub(crate) fn into_code(self) -> Result<Value, EvalError> {
        match self {
            Self::Scalar(ExpressionValue::Value(value)) => Ok(value),
            Self::Scalar(ExpressionValue::Empty) => Ok(Value::Int32(0)),
            Self::Scalar(ExpressionValue::Error(_)) | Self::Bytes(_) => {
                Err(EvalError::TypeMismatch)
            }
            Self::Scalar(ExpressionValue::Missing) => Err(EvalError::NonScalarExpression),
        }
    }

    pub(crate) fn into_string(self) -> Result<ByteOperand, EvalError> {
        match self {
            Self::Bytes(bytes) => Ok(ByteOperand::Bytes(bytes)),
            Self::Scalar(ExpressionValue::Value(value)) => Ok(ByteOperand::Scalar(value)),
            Self::Scalar(ExpressionValue::Empty) => {
                Ok(ByteOperand::Scalar(Value::Text(String::new())))
            }
            Self::Scalar(ExpressionValue::Error(_)) => Err(EvalError::TypeMismatch),
            Self::Scalar(ExpressionValue::Missing) => Err(EvalError::NonScalarExpression),
        }
    }
}

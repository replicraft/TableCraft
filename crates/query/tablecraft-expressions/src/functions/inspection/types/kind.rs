use super::InspectionInput;
use crate::{EvalError, arrays::ArrayView};
use tablecraft_core::values::{ExpressionValue, Value};

const ARRAY_FLAG: i16 = 8192;

#[derive(Clone, Copy)]
#[repr(i16)]
enum VariantKind {
    Empty = 0,
    Null = 1,
    Integer = 2,
    Long = 3,
    Single = 4,
    Double = 5,
    Currency = 6,
    Date = 7,
    String = 8,
    Error = 10,
    Boolean = 11,
    Variant = 12,
    Decimal = 14,
    Byte = 17,
    LongLong = 20,
}

impl VariantKind {
    fn name(self) -> &'static str {
        match self {
            Self::Empty => "Empty",
            Self::Null => "Null",
            Self::Integer => "Integer",
            Self::Long => "Long",
            Self::Single => "Single",
            Self::Double => "Double",
            Self::Currency => "Currency",
            Self::Date => "Date",
            Self::String => "String",
            Self::Error => "Error",
            Self::Boolean => "Boolean",
            Self::Variant => "Variant",
            Self::Decimal => "Decimal",
            Self::Byte => "Byte",
            Self::LongLong => "LongLong",
        }
    }
}

pub(super) fn metadata(input: InspectionInput<'_>) -> Result<(&'static str, i16), EvalError> {
    let kind = match input {
        InspectionInput::ByteString(_) => VariantKind::String,
        InspectionInput::Array(ArrayView::Variants(_)) => {
            return Ok(("Variant()", ARRAY_FLAG + VariantKind::Variant as i16));
        }
        InspectionInput::Array(ArrayView::Bytes(_)) => {
            return Ok(("Byte()", ARRAY_FLAG + VariantKind::Byte as i16));
        }
        InspectionInput::Expression(ExpressionValue::Empty) => VariantKind::Empty,
        InspectionInput::Expression(ExpressionValue::Error(_)) => VariantKind::Error,
        InspectionInput::Expression(ExpressionValue::Missing) => {
            return Err(EvalError::UnsupportedExpression);
        }
        InspectionInput::Expression(ExpressionValue::Value(value)) => {
            return metadata(InspectionInput::Scalar(value));
        }
        InspectionInput::Scalar(value) => match value {
            Value::Null => VariantKind::Null,
            Value::Int16(_) => VariantKind::Integer,
            Value::Int32(_) => VariantKind::Long,
            Value::Single(_) => VariantKind::Single,
            Value::Double(_) => VariantKind::Double,
            Value::Currency(_) => VariantKind::Currency,
            Value::DateTime(_) => VariantKind::Date,
            Value::Text(_) => VariantKind::String,
            Value::Boolean(_) => VariantKind::Boolean,
            Value::Decimal(_) => VariantKind::Decimal,
            Value::Byte(_) => VariantKind::Byte,
            Value::Int64(_) => VariantKind::LongLong,
            Value::Binary(_) => {
                return Ok(("Byte()", ARRAY_FLAG + VariantKind::Byte as i16));
            }
            Value::Guid(_) | Value::DateTimeExtended(_) => return Err(EvalError::TypeMismatch),
        },
    };

    Ok((kind.name(), kind as i16))
}

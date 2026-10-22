//! & returns Text unless both operands are NULL; formatting stays execution-local.
use crate::OutputType;
use tablecraft_schema::ScalarType;

pub(super) fn result(left: OutputType, right: OutputType) -> OutputType {
    if left == OutputType::Null && right == OutputType::Null {
        return OutputType::Null;
    }
    match (nullable(left), nullable(right)) {
        (Some(left), Some(right)) => OutputType::scalar(ScalarType::Text, left && right),
        _ => OutputType::Dynamic,
    }
}

fn nullable(input: OutputType) -> Option<bool> {
    match input {
        OutputType::Null => Some(true),
        OutputType::Scalar {
            data_type,
            nullable,
        } if matches!(
            data_type.scalar_type(),
            ScalarType::Boolean
                | ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Single
                | ScalarType::Double
                | ScalarType::Currency
                | ScalarType::Decimal
                | ScalarType::Text
                | ScalarType::DateTime
                | ScalarType::DateTimeExtended
        ) =>
        {
            Some(nullable)
        }
        _ => None,
    }
}

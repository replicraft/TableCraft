//! Unary operators whose successful materialized carrier does not depend on value.
use crate::OutputType;
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::UnaryOperator;

pub(super) fn result(operator: UnaryOperator, input: OutputType) -> OutputType {
    if input == OutputType::Null {
        return OutputType::Null;
    }
    let OutputType::Scalar {
        data_type,
        nullable,
    } = input
    else {
        return OutputType::Dynamic;
    };
    let scalar = data_type.scalar_type();
    match (operator, scalar) {
        (UnaryOperator::Plus | UnaryOperator::Minus, ScalarType::Text) => {
            OutputType::scalar(ScalarType::Double, nullable)
        }
        (
            UnaryOperator::Plus,
            ScalarType::Boolean
            | ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Currency
            | ScalarType::Decimal
            | ScalarType::DateTime,
        ) => input,
        (UnaryOperator::Minus, ScalarType::Byte | ScalarType::Boolean) => {
            OutputType::scalar(ScalarType::Int16, nullable)
        }
        (
            UnaryOperator::Minus,
            ScalarType::Int64
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Currency
            | ScalarType::Decimal,
        ) => OutputType::scalar(scalar, nullable),
        // Integer/Long negation may widen; NOT retains its separate carrier rules.
        _ => OutputType::Dynamic,
    }
}

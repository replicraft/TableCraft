//! Describe the implemented accumulator, without evaluating its argument.
use crate::{OutputType, plan::Aggregate};
use tablecraft_schema::ScalarType;

pub(super) fn numeric(kind: Aggregate, input: OutputType) -> OutputType {
    if input == OutputType::Null {
        return OutputType::Null;
    }
    let OutputType::Scalar { data_type, .. } = input else {
        return OutputType::Dynamic;
    };
    let scalar = match (kind, data_type.scalar_type()) {
        (
            Aggregate::Sum,
            ScalarType::Boolean
            | ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64,
        ) => ScalarType::Int64,
        (Aggregate::Sum | Aggregate::Average, ScalarType::Single | ScalarType::Double) => {
            ScalarType::Double
        }
        (
            Aggregate::Average,
            ScalarType::Boolean
            | ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int32
            | ScalarType::Int64,
        ) => ScalarType::Double,
        (Aggregate::Sum | Aggregate::Average, ScalarType::Decimal) => ScalarType::Decimal,
        (Aggregate::Sum, ScalarType::Currency) => ScalarType::Currency,
        (Aggregate::Average, ScalarType::Currency | ScalarType::DateTime) => ScalarType::Double,
        // Unsupported operand/result combinations do not acquire a descriptor.
        _ => return OutputType::Dynamic,
    };
    // Input constraints do not constrain a sum or mean. Empty groups allow NULL.
    OutputType::scalar(scalar, true)
}

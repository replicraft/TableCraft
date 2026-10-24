//! DateAdd successful carriers after the existing execution-local source conversion.
use crate::OutputType;
use tablecraft_schema::ScalarType;

pub(super) fn addition(input: OutputType) -> OutputType {
    let OutputType::Scalar {
        data_type,
        nullable,
    } = input
    else {
        return input;
    };

    let carrier = match data_type.scalar_type() {
        ScalarType::DateTimeExtended => ScalarType::DateTimeExtended,
        ScalarType::DateTime
        | ScalarType::Boolean
        | ScalarType::Byte
        | ScalarType::Int16
        | ScalarType::Int32
        | ScalarType::Int64
        | ScalarType::Single
        | ScalarType::Double
        | ScalarType::Currency
        | ScalarType::Decimal
        | ScalarType::Text => ScalarType::DateTime,
        _ => return OutputType::Dynamic,
    };

    // Controls may fail, but only a NULL source can return a successful NULL.
    // Formatting, range checks and actual conversion belong to execution.
    OutputType::scalar(carrier, nullable)
}

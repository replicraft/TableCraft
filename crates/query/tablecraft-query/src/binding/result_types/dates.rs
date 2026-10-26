//! Successful date carriers and NULL propagation without execution-local conversion.
use crate::OutputType;
use tablecraft_expressions::functions::dates::parsing::DateConversion;
use tablecraft_schema::ScalarType;

pub(super) fn conversion(function: DateConversion, input: OutputType) -> OutputType {
    match function {
        // NULL raises InvalidNullConversion rather than returning a NULL value.
        DateConversion::CDate | DateConversion::CVDate => {
            OutputType::scalar(ScalarType::DateTime, false)
        }
        DateConversion::DateValue | DateConversion::TimeValue => {
            propagating(input, ScalarType::DateTime)
        }
    }
}

pub(super) fn propagating(input: OutputType, carrier: ScalarType) -> OutputType {
    match input {
        OutputType::Null => OutputType::Null,
        OutputType::Scalar { nullable, .. } => OutputType::scalar(carrier, nullable),
        OutputType::Dynamic => OutputType::scalar(carrier, true),
        OutputType::Multivalue { .. } => OutputType::Dynamic,
    }
}

pub(super) fn difference(first: OutputType, second: OutputType) -> OutputType {
    if first == OutputType::Null || second == OutputType::Null {
        return OutputType::Null;
    }
    let nonnull = |input| {
        matches!(
            input,
            OutputType::Scalar {
                nullable: false,
                ..
            }
        )
    };
    OutputType::scalar(ScalarType::Int32, !nonnull(first) || !nonnull(second))
}

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

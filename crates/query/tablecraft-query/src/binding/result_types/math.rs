//! Successful scalar carriers; this does not execute functions or infer coercions.
use crate::OutputType;
use tablecraft_expressions::functions::math::MathFunction;
use tablecraft_schema::ScalarType;

pub(super) fn result(function: MathFunction, input: Option<OutputType>) -> OutputType {
    // These functions propagate a NULL value operand on every successful call.
    // Metadata does not bypass eager operands or Round's digit validation.
    if input == Some(OutputType::Null)
        && !matches!(
            function,
            MathFunction::Rnd | MathFunction::Rgb | MathFunction::QbColor
        )
    {
        return OutputType::Null;
    }

    match function {
        MathFunction::Sgn => OutputType::scalar(ScalarType::Int16, true),
        MathFunction::Sqr
        | MathFunction::Exp
        | MathFunction::Log
        | MathFunction::Sin
        | MathFunction::Cos
        | MathFunction::Tan
        | MathFunction::Atn => OutputType::scalar(ScalarType::Double, true),
        MathFunction::Rnd => OutputType::scalar(ScalarType::Single, false),
        MathFunction::Rgb | MathFunction::QbColor => OutputType::scalar(ScalarType::Int32, false),
        MathFunction::Abs | MathFunction::Int | MathFunction::Fix | MathFunction::Round => {
            let Some(OutputType::Scalar { data_type, .. }) = input else {
                return OutputType::Dynamic;
            };
            let scalar = match data_type.scalar_type() {
                ScalarType::Boolean => ScalarType::Int16,
                ScalarType::Text => ScalarType::Double,
                ScalarType::DateTime if function == MathFunction::Abs => ScalarType::Double,
                ScalarType::DateTime
                    if matches!(function, MathFunction::Int | MathFunction::Fix) =>
                {
                    ScalarType::DateTime
                }
                scalar @ (ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Single
                | ScalarType::Double
                | ScalarType::Currency
                | ScalarType::Decimal) => scalar,
                _ => return OutputType::Dynamic,
            };
            // Changed values can violate the operand's precision/scale constraints.
            OutputType::scalar(scalar, true)
        }
    }
}

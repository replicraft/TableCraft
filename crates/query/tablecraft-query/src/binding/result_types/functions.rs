//! Successful scalar carriers; this does not execute functions or infer coercions.
use crate::OutputType;
use tablecraft_expressions::functions::{math::MathFunction, text::TextFunction};
use tablecraft_schema::ScalarType;

pub(super) fn text(function: TextFunction) -> OutputType {
    let scalar = match function {
        TextFunction::Len
        | TextFunction::LenB
        | TextFunction::InStr
        | TextFunction::InStrB
        | TextFunction::InStrRev => ScalarType::Int32,
        TextFunction::StrComp | TextFunction::Asc | TextFunction::AscB | TextFunction::AscW => {
            ScalarType::Int16
        }
        TextFunction::Val => ScalarType::Double,
        // StrConv can return raw bytes or Unicode depending on its controls.
        TextFunction::ChrB
        | TextFunction::LeftB
        | TextFunction::RightB
        | TextFunction::MidB
        | TextFunction::StrConv => return OutputType::Dynamic,
        TextFunction::LCase
        | TextFunction::UCase
        | TextFunction::LTrim
        | TextFunction::RTrim
        | TextFunction::Trim
        | TextFunction::Left
        | TextFunction::Right
        | TextFunction::Mid
        | TextFunction::Replace
        | TextFunction::Space
        | TextFunction::String
        | TextFunction::StrReverse
        | TextFunction::Chr
        | TextFunction::ChrW
        | TextFunction::Hex
        | TextFunction::Oct
        | TextFunction::Str
        | TextFunction::CStr
        | TextFunction::Format
        | TextFunction::FormatDateTime
        | TextFunction::MonthName
        | TextFunction::WeekdayName
        | TextFunction::FormatNumber
        | TextFunction::FormatPercent
        | TextFunction::FormatCurrency
        | TextFunction::Partition
        | TextFunction::HyperlinkPart => ScalarType::Text,
    };
    OutputType::scalar(scalar, true)
}

pub(super) fn math(function: MathFunction, input: Option<OutputType>) -> OutputType {
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

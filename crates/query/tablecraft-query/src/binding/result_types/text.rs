//! Text-call result metadata from existing successful-value and NULL contracts.
use crate::OutputType;
use tablecraft_expressions::functions::text::TextFunction;
use tablecraft_schema::ScalarType;

/// Argument positions in the already normalized call, not written SQL slots.
/// Controls remain eager runtime work even when a source is NULL-only.
pub(super) fn null_inputs(function: TextFunction, width: usize) -> &'static [usize] {
    match function {
        TextFunction::Len
        | TextFunction::LenB
        | TextFunction::LCase
        | TextFunction::UCase
        | TextFunction::LTrim
        | TextFunction::RTrim
        | TextFunction::Trim
        | TextFunction::Left
        | TextFunction::Right
        | TextFunction::Mid
        | TextFunction::LeftB
        | TextFunction::RightB
        | TextFunction::MidB
        | TextFunction::StrConv
        | TextFunction::Space
        | TextFunction::Hex
        | TextFunction::Oct
        | TextFunction::Str => &[0],
        TextFunction::String | TextFunction::StrComp | TextFunction::InStrRev => &[0, 1],
        TextFunction::InStr | TextFunction::InStrB if width == 2 => &[0, 1],
        TextFunction::InStr | TextFunction::InStrB => &[1, 2],
        _ => &[],
    }
}

pub(super) fn result(function: TextFunction) -> OutputType {
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
        | TextFunction::ChrString
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

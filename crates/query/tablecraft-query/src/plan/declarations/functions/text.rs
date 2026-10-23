//! Formal query-call return categories, independent of returned Text/number bits.
use tablecraft_expressions::{functions::text::TextFunction, numeric::OperandDeclaration};

pub(super) fn result(function: TextFunction) -> OperandDeclaration {
    match function {
        TextFunction::Asc
        | TextFunction::AscB
        | TextFunction::AscW
        | TextFunction::InStrRev
        | TextFunction::Val
        | TextFunction::CStr
        | TextFunction::ChrString
        | TextFunction::Replace
        | TextFunction::StrReverse
        | TextFunction::FormatDateTime
        | TextFunction::MonthName
        | TextFunction::WeekdayName
        | TextFunction::FormatNumber
        | TextFunction::FormatPercent
        | TextFunction::FormatCurrency
        | TextFunction::HyperlinkPart => OperandDeclaration::Scalar,
        TextFunction::Len
        | TextFunction::LenB
        | TextFunction::Chr
        | TextFunction::ChrW
        | TextFunction::ChrB
        | TextFunction::LeftB
        | TextFunction::RightB
        | TextFunction::MidB
        | TextFunction::StrConv
        | TextFunction::LCase
        | TextFunction::UCase
        | TextFunction::LTrim
        | TextFunction::RTrim
        | TextFunction::Trim
        | TextFunction::Left
        | TextFunction::Right
        | TextFunction::Mid
        | TextFunction::StrComp
        | TextFunction::InStr
        | TextFunction::InStrB
        | TextFunction::Space
        | TextFunction::String
        | TextFunction::Hex
        | TextFunction::Oct
        | TextFunction::Str
        | TextFunction::Format
        | TextFunction::Partition => OperandDeclaration::Variant,
    }
}

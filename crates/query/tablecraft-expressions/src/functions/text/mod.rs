//! Access text/conversion built-ins with explicit policies and metered output.
use crate::{EvalError, comparison::Collation};
pub mod byte_string;
pub mod calendar_names;
pub mod casing;
mod characters;
pub(crate) mod checked_buffer;
pub mod code_page;
mod compare;
mod context;
pub mod cstr;
pub mod date_format;
mod format_buffer;
pub mod hyperlink;
pub use context::TextContext;
pub mod construction;
pub mod numeric_format;
pub mod output;
pub mod partition;
pub mod replacement;
mod search;
#[cfg(feature = "sql")]
pub mod sql_arguments;
pub mod styled_number;
pub mod val;
use tablecraft_core::values::Value;

/// Closed text operations with explicit comparison, encoding and output policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFunction {
    /// UTF-16 code-unit length, returned as Int32.
    Len,
    /// UTF-16 byte length of Unicode Text, returned as Int32.
    LenB,
    /// Construct a single raw byte, retained as a string intermediate.
    ChrB,
    /// Select leading raw string bytes.
    LeftB,
    /// Select trailing raw string bytes.
    RightB,
    /// Select raw string bytes from a one-based byte position.
    MidB,
    /// Lowercase Text under an explicit Unicode language profile.
    LCase,
    /// Uppercase Text under an explicit Unicode language profile.
    UCase,
    /// Remove leading U+0020 spaces.
    LTrim,
    /// Remove trailing U+0020 spaces.
    RTrim,
    /// Remove leading and trailing U+0020 spaces.
    Trim,
    /// Select the first length UTF-16 units.
    Left,
    /// Select the last length UTF-16 units.
    Right,
    /// Select from a one-based UTF-16 position, with optional length.
    Mid,
    /// Compare two strings, returning Int16 -1/0/1 or NULL.
    StrComp,
    /// First substring position as a one-based Int32, or zero/NULL.
    InStr,
    /// First raw byte substring position under an explicit supported policy.
    InStrB,
    /// Last substring position before an end boundary, as Int32, or zero/NULL.
    InStrRev,
    /// Bounded non-overlapping substring replacement over a returned suffix.
    Replace,
    /// Repeat a checked number of U+0020 spaces.
    Space,
    /// Repeat the first UTF-16 unit or an explicitly decoded numeric byte code.
    String,
    /// Reverse BMP code units without inferring grapheme/locale behavior.
    StrReverse,
    /// Construct a byte-code character using the explicit encoding policy.
    Chr,
    /// First character byte code in the explicit encoding policy.
    Asc,
    /// First raw UTF-16LE byte of Text, returned as Int16.
    AscB,
    /// Construct one representable UTF-16 code unit.
    ChrW,
    /// First UTF-16 code unit as a signed Integer.
    AscW,
    /// Parse a locale-independent numeric prefix under a charged scratch buffer.
    Val,
    /// Uppercase hexadecimal spelling of a checked integral conversion.
    Hex,
    /// Octal spelling of a checked integral conversion.
    Oct,
    /// Locale-independent numeric spelling with a reserved sign space.
    Str,
    /// Explicit Text conversion under selected numeric/date output policies.
    CStr,
    /// Render a checked date/time using one of the five Access named formats.
    FormatDateTime,
    /// Explicit-language month name for a checked numeric designation.
    MonthName,
    /// Explicit-language weekday name under selected first-day numbering.
    WeekdayName,
    /// Fixed-place numeric formatting under an explicit regional profile.
    FormatNumber,
    /// Percentage formatting with decimal-point display scaling.
    FormatPercent,
    /// Currency formatting under explicit symbol/placement policy.
    FormatCurrency,
    /// Padded numeric bucket labels with four required operands.
    Partition,
    /// Extract one stored hyperlink component using an explicit SQL selector.
    HyperlinkPart,
}
impl TextFunction {
    /// Largest accepted argument count; used for fixed-size evaluation storage.
    pub const MAX_ARGUMENTS: usize = 6;
    /// Resolve only ASCII-insensitive unqualified names.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("Len", Self::Len),
            ("LenB", Self::LenB),
            ("ChrB", Self::ChrB),
            ("LeftB", Self::LeftB),
            ("RightB", Self::RightB),
            ("MidB", Self::MidB),
            ("LCase", Self::LCase),
            ("UCase", Self::UCase),
            ("LTrim", Self::LTrim),
            ("RTrim", Self::RTrim),
            ("Trim", Self::Trim),
            ("Left", Self::Left),
            ("Right", Self::Right),
            ("Mid", Self::Mid),
            ("StrComp", Self::StrComp),
            ("InStr", Self::InStr),
            ("InStrB", Self::InStrB),
            ("InStrRev", Self::InStrRev),
            ("Replace", Self::Replace),
            ("Space", Self::Space),
            ("String", Self::String),
            ("StrReverse", Self::StrReverse),
            ("Chr", Self::Chr),
            ("Asc", Self::Asc),
            ("AscB", Self::AscB),
            ("ChrW", Self::ChrW),
            ("AscW", Self::AscW),
            ("Val", Self::Val),
            ("Hex", Self::Hex),
            ("Oct", Self::Oct),
            ("Str", Self::Str),
            ("CStr", Self::CStr),
            ("FormatDateTime", Self::FormatDateTime),
            ("MonthName", Self::MonthName),
            ("WeekdayName", Self::WeekdayName),
            ("FormatNumber", Self::FormatNumber),
            ("FormatPercent", Self::FormatPercent),
            ("FormatCurrency", Self::FormatCurrency),
            ("Partition", Self::Partition),
            ("HyperlinkPart", Self::HyperlinkPart),
        ]
        .into_iter()
        .find_map(|(spelling, operation)| name.eq_ignore_ascii_case(spelling).then_some(operation))
    }
    /// Whether this operation produces a lossless byte-string intermediate.
    pub const fn produces_bytes(self) -> bool {
        matches!(self, Self::ChrB | Self::LeftB | Self::RightB | Self::MidB)
    }

    /// Validate syntax shape before evaluating operands.
    pub const fn accepts_arity(self, count: usize) -> bool {
        match self {
            Self::Len
            | Self::LenB
            | Self::ChrB
            | Self::LCase
            | Self::UCase
            | Self::LTrim
            | Self::RTrim
            | Self::Trim
            | Self::Space
            | Self::StrReverse
            | Self::Chr
            | Self::Asc
            | Self::AscB
            | Self::ChrW
            | Self::AscW
            | Self::Val
            | Self::Hex
            | Self::Oct
            | Self::Str
            | Self::CStr => count == 1,
            Self::Partition => count == partition::Partition::ARGUMENTS,
            Self::Left
            | Self::Right
            | Self::LeftB
            | Self::RightB
            | Self::String
            | Self::HyperlinkPart => count == 2,
            Self::Mid | Self::MidB | Self::StrComp => count == 2 || count == 3,
            Self::FormatDateTime | Self::MonthName => count == 1 || count == 2,
            Self::WeekdayName => count >= 1 && count <= 3,
            Self::FormatNumber | Self::FormatPercent | Self::FormatCurrency => {
                count >= 1 && count <= 5
            }
            Self::InStr | Self::InStrB | Self::InStrRev => count >= 2 && count <= 4,
            Self::Replace => count >= 3 && count <= Self::MAX_ARGUMENTS,
        }
    }
}
/// Borrowed output; hosts must meter and fallibly copy Text before materializing.
#[derive(Debug, PartialEq, Eq)]
pub enum TextResult<'a> {
    /// Borrowed casing recipe, streamed under an explicit language.
    Casing(casing::Casing<'a>),
    /// SQL NULL, distinct from zero-length text.
    Null,
    /// Exact Int32 UTF-16 length.
    Length(i32),
    /// One-based Int32 substring position, or zero when not found.
    Position(i32),
    /// Signed Int16 string ordering result.
    Comparison(i16),
    /// Exact signed Int16 character code.
    CharacterCode(i16),
    /// One representable Unicode character; hosts charge its UTF-8 output.
    Character(char),
    /// Borrowed numeric-prefix recipe, evaluated with charged normalization storage.
    NumericPrefix(val::NumericPrefix<'a>),
    /// Borrowed numeric-to-text recipe using bounded stack scratch.
    NumericFormat(numeric_format::NumericFormat<'a>),
    /// Stack-only bucket recipe with bounded, charged ASCII output.
    Partition(partition::Partition),
    /// Borrowed date-to-Text recipe under an explicit output profile.
    DateFormat(cstr::DateText<'a>),
    /// Date conversion followed by named-format output using separate charged buffers.
    DateTimeFormat(date_format::DateTimeFormat<'a>),
    /// Validated month/weekday name; emit through bounded TextOutput.
    CalendarName(calendar_names::CalendarName),
    /// Fixed-place output after private numeric Text normalization.
    StyledNumber(styled_number::StyledNumber<'a>),
    /// A valid Unicode slice of an input string, without allocation.
    Text(&'a str),
    /// Borrowed replacement recipe; emit through a bounded host TextOutput.
    Replacement(replacement::Replacement<'a>),
    /// Borrowed bounded repeat/reverse recipe, written through TextOutput.
    Construction(construction::Construction<'a>),
}
/// Apply a text operation over evaluated operands, invoking `step` per scanned
/// Unicode scalar (UTF-16 unit for comparison/search). Positions count UTF-16 units;
/// slicing across a surrogate pair fails rather than producing replacement characters. Only Text/NULL input and exact
/// Byte/Int16/Int32/Int64 positional operands in Long range are supported. StrComp
/// and substring search instead use checked numeric-to-Long conversion for modes
/// and starts, with explicit function-specific defaults. Replace returns a borrowed
/// recipe; its write method requires a bounded TextOutput and comparison context.
pub fn apply<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    apply_with_collation(function, arguments, None, step)
}
/// Apply text functions with an explicit database/default/text comparison host.
/// Existing non-comparison functions ignore this context. Hosts still own callback
/// work limits; string inputs and binary comparisons are scanned through step.
pub fn apply_with_collation<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    collation: Option<&dyn Collation>,
    step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    apply_with_context(
        function,
        arguments,
        TextContext {
            collation,
            ..Default::default()
        },
        step,
    )
}
/// Apply text functions with explicit comparison, encoding and conversion output policies.
/// Borrowed construction/replacement results must be emitted through a bounded sink.
pub fn apply_with_context<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    context: TextContext<'_>,
    mut step: impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'a>, EvalError> {
    let collation = context.collation;
    if !function.accepts_arity(arguments.len()) {
        return Err(EvalError::UnsupportedExpression);
    }
    step()?;
    if function.produces_bytes() || function == TextFunction::InStrB {
        // Byte producers use the owning byte-operand evaluator, never a borrowed
        // TextResult that could discard odd bytes or unpaired UTF-16 units.
        return Err(EvalError::UnsupportedExpression);
    }
    if matches!(
        function,
        TextFunction::FormatNumber | TextFunction::FormatPercent | TextFunction::FormatCurrency
    ) {
        return styled_number::prepare(function, arguments, context).map(TextResult::StyledNumber);
    }
    if function == TextFunction::CStr {
        return cstr::prepare(&arguments[0], context);
    }
    if matches!(
        function,
        TextFunction::MonthName | TextFunction::WeekdayName
    ) {
        return calendar_names::prepare(function, arguments, context).map(TextResult::CalendarName);
    }
    if function == TextFunction::FormatDateTime {
        return date_format::prepare(arguments, context).map(TextResult::DateTimeFormat);
    }
    if matches!(
        function,
        TextFunction::Hex | TextFunction::Oct | TextFunction::Str
    ) {
        return numeric_format::prepare(function, &arguments[0]);
    }
    if function == TextFunction::HyperlinkPart {
        return hyperlink::apply(arguments, &mut step);
    }
    if function == TextFunction::Partition {
        return partition::prepare(arguments);
    }
    if function == TextFunction::Val {
        return val::prepare(&arguments[0]);
    }
    if matches!(
        function,
        TextFunction::Chr
            | TextFunction::Asc
            | TextFunction::AscB
            | TextFunction::ChrW
            | TextFunction::AscW
    ) {
        return characters::apply(function, &arguments[0], context);
    }
    if matches!(
        function,
        TextFunction::Space | TextFunction::String | TextFunction::StrReverse
    ) {
        return construction::prepare(function, arguments, context.character_code_page);
    }
    if function == TextFunction::Replace {
        return Ok(TextResult::Replacement(replacement::prepare(arguments)?));
    }
    if function == TextFunction::StrComp {
        return compare::apply(arguments, collation, &mut step);
    }
    if matches!(function, TextFunction::InStr | TextFunction::InStrRev) {
        return search::apply(function, arguments, collation, &mut step);
    }
    let text = match &arguments[0] {
        Value::Null => return Ok(TextResult::Null),
        Value::Text(text) => text.as_str(),
        _ => return Err(EvalError::TypeMismatch),
    };
    match function {
        TextFunction::ChrB
        | TextFunction::LeftB
        | TextFunction::RightB
        | TextFunction::MidB
        | TextFunction::StrComp
        | TextFunction::InStr
        | TextFunction::InStrB
        | TextFunction::InStrRev
        | TextFunction::Replace
        | TextFunction::Space
        | TextFunction::String
        | TextFunction::StrReverse
        | TextFunction::Chr
        | TextFunction::Asc
        | TextFunction::AscB
        | TextFunction::ChrW
        | TextFunction::AscW
        | TextFunction::Val
        | TextFunction::Hex
        | TextFunction::Oct
        | TextFunction::Str
        | TextFunction::CStr
        | TextFunction::FormatDateTime
        | TextFunction::MonthName
        | TextFunction::WeekdayName
        | TextFunction::FormatNumber
        | TextFunction::FormatPercent
        | TextFunction::FormatCurrency
        | TextFunction::Partition
        | TextFunction::HyperlinkPart => {
            unreachable!("handled before single-text functions")
        }
        TextFunction::LCase | TextFunction::UCase => Ok(TextResult::Casing(casing::Casing::new(
            text,
            function == TextFunction::UCase,
        ))),
        TextFunction::Len => Ok(TextResult::Length(units(text, &mut step)?)),
        TextFunction::LenB => Ok(TextResult::Length(byte_string::length(text, &mut step)?)),
        TextFunction::LTrim | TextFunction::RTrim | TextFunction::Trim => {
            let mut result = text;
            if function != TextFunction::RTrim {
                for ch in text.chars() {
                    step()?;
                    if ch != ' ' {
                        break;
                    }
                    result = &result[1..];
                }
            }
            if function != TextFunction::LTrim {
                let mut end = result.len();
                for ch in result.chars().rev() {
                    step()?;
                    if ch != ' ' {
                        break;
                    }
                    end -= 1;
                }
                result = &result[..end];
            }
            Ok(TextResult::Text(result))
        }
        TextFunction::Left | TextFunction::Right | TextFunction::Mid => {
            let position = long(&arguments[1])?;
            let length = if function == TextFunction::Mid {
                if position < 1 {
                    return Err(EvalError::Domain);
                }
                arguments.get(2).map(long).transpose()?
            } else {
                Some(position)
            };
            if length.is_some_and(|length| length < 0) {
                return Err(EvalError::Domain);
            }
            let total = units(text, &mut step)?;
            let (start, end) = match function {
                TextFunction::Left => (0, position.min(total)),
                TextFunction::Right => (total.saturating_sub(position).max(0), total),
                _ => {
                    let start = (position - 1).min(total);
                    (
                        start,
                        start.saturating_add(length.unwrap_or(total)).min(total),
                    )
                }
            };
            if start == end {
                return Ok(TextResult::Text(""));
            }
            let start = boundary(text, start, &mut step)?;
            let end = boundary(text, end, &mut step)?;
            Ok(TextResult::Text(&text[start..end]))
        }
    }
}
fn long(value: &Value) -> Result<i32, EvalError> {
    let integer = match value {
        Value::Byte(value) => i64::from(*value),
        Value::Int16(value) => i64::from(*value),
        Value::Int32(value) => i64::from(*value),
        Value::Int64(value) => *value,
        _ => return Err(EvalError::TypeMismatch),
    };
    integer.try_into().map_err(|_| EvalError::Overflow)
}
fn units(text: &str, step: &mut impl FnMut() -> Result<(), EvalError>) -> Result<i32, EvalError> {
    text.chars().try_fold(0_i32, |length, ch| {
        step()?;
        length
            .checked_add(ch.len_utf16() as i32)
            .ok_or(EvalError::Overflow)
    })
}
fn boundary(
    text: &str,
    position: i32,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<usize, EvalError> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        step()?;
        if units == position {
            return Ok(byte);
        }
        units += ch.len_utf16() as i32;
        if units > position {
            return Err(EvalError::UnrepresentableText);
        }
    }
    Ok(text.len())
}
#[cfg(test)]
mod tests;

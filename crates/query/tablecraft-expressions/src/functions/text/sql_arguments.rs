//! Stack-only lowering of supported optional text-function argument slots.
use super::TextFunction;
use tablecraft_sql::select::{Expression, FunctionArgument};

const FIRST_POSITION: i64 = 1;
const LAST_POSITION: i64 = -1;
const ALL_MATCHES: i64 = -1;
const DO_NOT_ABBREVIATE: i64 = 0;
const REGIONAL_PLACES: i64 = -1;
const REGIONAL_SWITCH: i64 = -2;

/// A borrowed written expression or an explicit function-specific default.
#[derive(Clone, Copy)]
pub enum Argument<'a> {
    /// Evaluate/bind this written expression in argument order.
    Expression(&'a Expression),
    /// A default integer; never a SQL NULL replacement.
    Integer(i64),
}
/// Normalize valid call shapes without cloning expressions or allocating. Invalid
/// arities, wildcards and missing required slots fail before operand evaluation.
pub fn normalize(
    function: TextFunction,
    arguments: &[FunctionArgument],
) -> Option<impl ExactSizeIterator<Item = Argument<'_>>> {
    if !function.accepts_arity(arguments.len()) {
        return None;
    }
    let mut arguments = arguments;
    if matches!(arguments.last(), Some(FunctionArgument::Missing))
        && matches!(
            (function, arguments.len()),
            (TextFunction::StrComp | TextFunction::StrConv, 3)
                | (TextFunction::InStr | TextFunction::InStrB, 4)
                | (TextFunction::InStrRev, 3 | 4)
                | (TextFunction::FormatDateTime, 2)
        )
    {
        arguments = &arguments[..arguments.len() - 1];
    }
    if function == TextFunction::InStrRev && matches!(arguments, [_, _, FunctionArgument::Missing])
    {
        arguments = &arguments[..2];
    }
    if matches!(
        function,
        TextFunction::MonthName | TextFunction::WeekdayName
    ) {
        while arguments.len() > 1 && matches!(arguments.last(), Some(FunctionArgument::Missing)) {
            arguments = &arguments[..arguments.len() - 1];
        }
    }
    if matches!(
        function,
        TextFunction::FormatNumber | TextFunction::FormatPercent | TextFunction::FormatCurrency
    ) {
        while arguments.len() > 1 && matches!(arguments.last(), Some(FunctionArgument::Missing)) {
            arguments = &arguments[..arguments.len() - 1];
        }
    }
    if function == TextFunction::Replace {
        while arguments.len() > 3 && matches!(arguments.last(), Some(FunctionArgument::Missing)) {
            arguments = &arguments[..arguments.len() - 1];
        }
    }
    let mut slots = [None; TextFunction::MAX_ARGUMENTS];
    let count = arguments.len();
    for (index, argument) in arguments.iter().enumerate() {
        slots[index] = Some(match argument {
            FunctionArgument::Expression(expression) => Argument::Expression(expression),
            FunctionArgument::Missing => match (function, index, arguments.len()) {
                (
                    TextFunction::FormatNumber
                    | TextFunction::FormatPercent
                    | TextFunction::FormatCurrency,
                    1,
                    _,
                ) => Argument::Integer(REGIONAL_PLACES),
                (
                    TextFunction::FormatNumber
                    | TextFunction::FormatPercent
                    | TextFunction::FormatCurrency,
                    2..=4,
                    _,
                ) => Argument::Integer(REGIONAL_SWITCH),
                (TextFunction::WeekdayName, 1, 3) => Argument::Integer(DO_NOT_ABBREVIATE),
                (TextFunction::InStr | TextFunction::InStrB, 0, 3) => {
                    Argument::Integer(FIRST_POSITION)
                }
                (TextFunction::InStrRev, 2, 4) => Argument::Integer(LAST_POSITION),
                (TextFunction::Replace, 3, _) => Argument::Integer(FIRST_POSITION),
                (TextFunction::Replace, 4, _) => Argument::Integer(ALL_MATCHES),
                _ => return None,
            },
            _ => return None,
        });
    }
    Some(
        slots
            .into_iter()
            .take(count)
            .map(|slot| slot.expect("validated text argument slots are contiguous")),
    )
}

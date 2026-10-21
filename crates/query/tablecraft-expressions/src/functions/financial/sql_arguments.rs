//! Borrowed optional-slot lowering shared by scalar and bound SQL evaluation.
use super::FinancialFunction;
use tablecraft_sql::select::{Expression, FunctionArgument};

/// Written operand or the numeric zero default for an omitted optional operand.
#[derive(Clone, Copy)]
pub enum Argument<'a> {
    /// Evaluate the supplied expression in its written position.
    Expression(&'a Expression),
    /// Omitted optional balance/timing; never represents explicit SQL NULL.
    Zero,
}
/// Validate required slots and arity without allocation, then supply optional
/// defaults. Missing/wildcard required arguments and any wildcard are rejected.
pub fn normalize(
    arguments: &[FunctionArgument],
) -> Option<impl ExactSizeIterator<Item = Argument<'_>>> {
    if !(FinancialFunction::REQUIRED_ARGUMENTS..=FinancialFunction::MAX_ARGUMENTS)
        .contains(&arguments.len())
    {
        return None;
    }
    for (index, argument) in arguments.iter().enumerate() {
        if !matches!(argument, FunctionArgument::Expression(_))
            && !(index >= FinancialFunction::REQUIRED_ARGUMENTS
                && matches!(argument, FunctionArgument::Missing))
        {
            return None;
        }
    }
    Some(arguments.iter().map(|argument| match argument {
        FunctionArgument::Expression(expr) => Argument::Expression(expr),
        _ => Argument::Zero,
    }))
}

//! Borrowed optional-slot lowering shared by scalar and bound SQL evaluation.
use super::FinancialFunction;
use tablecraft_sql::select::{Expression, FunctionArgument};

/// Written operand or the function-specific numeric default for an omitted optional operand.
#[derive(Clone, Copy)]
pub enum Argument<'a> {
    /// Evaluate the supplied expression in its written position.
    Expression(&'a Expression),
    /// Function-specific integer default; never represents explicit SQL NULL.
    Integer(i64),
}
/// Validate required slots and arity without allocation, then supply optional
/// defaults. Missing/wildcard required arguments and any wildcard are rejected.
pub fn normalize(
    function: FinancialFunction,
    arguments: &[FunctionArgument],
) -> Option<impl ExactSizeIterator<Item = Argument<'_>>> {
    if !function.accepts_arity(arguments.len()) {
        return None;
    }
    for (index, argument) in arguments.iter().enumerate() {
        if !matches!(argument, FunctionArgument::Expression(_))
            && !(index >= function.required_arguments()
                && matches!(argument, FunctionArgument::Missing))
        {
            return None;
        }
    }
    Some(arguments.iter().map(move |argument| match argument {
        FunctionArgument::Expression(expr) => Argument::Expression(expr),
        _ => Argument::Integer(if function == FinancialFunction::DDB {
            super::DEFAULT_DECLINE_FACTOR
        } else {
            0
        }),
    }))
}

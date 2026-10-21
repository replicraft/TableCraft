//! Recognize explicit cashflow Array constructors without evaluating or allocating.
use super::CashFlowFunction;
use crate::arrays::sql_arguments as array_arguments;
use tablecraft_sql::select::{Expression, FunctionArgument};
/// Validate the function shape and return the explicit ordered array elements.
/// Only Array(...) or VBA.Array(...) are recognized; no provider is activated.
/// IRR's optional omitted guess is allowed. Empty arrays remain explicit inputs.
pub fn elements(
    function: CashFlowFunction,
    arguments: &[FunctionArgument],
) -> Option<&[FunctionArgument]> {
    if !function.accepts_arity(arguments.len()) {
        return None;
    }
    let FunctionArgument::Expression(Expression::Call {
        qualifier,
        name,
        arguments: values,
    }) = &arguments[function.array_index()]
    else {
        return None;
    };
    if !array_arguments::is_constructor(qualifier, name) {
        return None;
    }
    let values = array_arguments::elements(values)?;

    for (index, value) in arguments.iter().enumerate() {
        if index != function.array_index()
            && !matches!(value, FunctionArgument::Expression(_))
            && !(function == CashFlowFunction::IRR
                && index == 1
                && matches!(value, FunctionArgument::Missing))
        {
            return None;
        }
    }
    Some(values)
}

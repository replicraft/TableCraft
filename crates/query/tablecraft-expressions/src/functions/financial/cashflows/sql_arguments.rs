//! Recognize explicit cashflow Array constructors without evaluating or allocating.
use super::{CashFlowFunction, CashFlows};
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
    if !name.as_str().eq_ignore_ascii_case("Array")
        || !(qualifier.is_empty()
            || (qualifier.len() == 1 && qualifier[0].as_str().eq_ignore_ascii_case("VBA")))
        || values.len() > CashFlows::MAX_ELEMENTS
    {
        return None;
    }
    if values
        .iter()
        .any(|v| !matches!(v, FunctionArgument::Expression(_)))
    {
        return None;
    }
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

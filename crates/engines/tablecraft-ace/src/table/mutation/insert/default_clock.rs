//! One explicit operation reading; projections never call the host again.
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockFunction, ClockReading},
};

pub(super) fn capture(
    provider: &dyn Clock,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ClockReading, EditError> {
    check(budget, cancelled)?;
    let reading = provider.read().map_err(evaluation_error)?;
    check(budget, cancelled)?;
    Ok(reading)
}

pub(super) fn project(
    function: ClockFunction,
    reading: ClockReading,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    if !matches!(function, ClockFunction::Date | ClockFunction::Now) {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    // ClockReading's Clock implementation is a pure copy, not the host callback.
    let value = function.apply(Some(&reading)).map_err(evaluation_error)?;
    check(budget, cancelled)?;
    Ok(value)
}

fn evaluation_error(error: EvalError) -> EditError {
    match error {
        EvalError::Cancelled => InspectError::Cancelled.into(),
        EvalError::Limit(error) => InspectError::Limit(error).into(),
        EvalError::Allocation => InspectError::Allocation.into(),
        error => EditError::DefaultEvaluation(error),
    }
}

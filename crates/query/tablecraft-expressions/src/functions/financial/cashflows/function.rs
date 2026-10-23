//! Closed function identities and numeric argument conversion.
use super::{CashFlows, Work, finite, number};
use crate::EvalError;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Value},
};

/// Closed cashflow functions, distinct from scalar financial argument lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CashFlowFunction {
    /// End-of-period net present value.
    NPV,
    /// Period-zero-based internal rate of return.
    IRR,
    /// Modified internal rate with separate financing/reinvestment rates.
    MIRR,
}

impl CashFlowFunction {
    /// Maximum scalar controls beside the explicit array argument.
    pub const MAX_SCALAR_ARGUMENTS: usize = 2;

    /// ASCII-insensitive lookup without resolving application code.
    pub fn resolve(name: &str) -> Option<Self> {
        [("NPV", Self::NPV), ("IRR", Self::IRR), ("MIRR", Self::MIRR)]
            .into_iter()
            .find_map(|(n, f)| name.eq_ignore_ascii_case(n).then_some(f))
    }

    /// Position of the array in the written function arguments.
    pub const fn array_index(self) -> usize {
        match self {
            Self::NPV => 1,
            Self::IRR | Self::MIRR => 0,
        }
    }

    /// Whether the written function arity is valid, including its array argument.
    pub const fn accepts_arity(self, count: usize) -> bool {
        match self {
            Self::NPV => count == 2,
            Self::IRR => count == 1 || count == 2,
            Self::MIRR => count == 3,
        }
    }

    /// Apply evaluated scalar operands and array elements under one budget.
    /// Callers evaluate all supplied expressions in written order beforehand.
    /// Numeric conversion then follows array/rate argument order. No array is
    /// inferred from a scalar or NULL; the sequence is explicitly supplied.
    pub fn apply(
        self,
        arguments: &[Value],
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Value, EvalError> {
        self.apply_with_input(
            arguments.len(),
            &mut |index| Ok(&arguments[index]),
            values.len(),
            &mut |index| Ok(&values[index]),
            &mut Work { budget, cancelled },
            &mut |value, _| number(value),
        )
    }

    pub(crate) fn apply_with_input<'a>(
        self,
        scalar_count: usize,
        scalar: &mut impl FnMut(usize) -> Result<&'a Value, EvalError>,
        element_count: usize,
        element: &mut impl FnMut(usize) -> Result<&'a Value, EvalError>,
        work: &mut Work<'_>,
        convert: &mut impl FnMut(&Value, &mut ResourceBudget) -> Result<f64, EvalError>,
    ) -> Result<Value, EvalError> {
        if !self.accepts_arity(scalar_count + 1) {
            return Err(EvalError::UnsupportedExpression);
        }

        let result = match self {
            Self::NPV => {
                let rate = convert(scalar(0)?, work.budget)?;
                let flows = CashFlows::new_with_input(element_count, element, work)?;
                flows.npv(rate, work.budget, work.cancelled)?
            }
            Self::IRR => {
                let flows = CashFlows::new_with_input(element_count, element, work)?;
                let guess = if scalar_count == 0 {
                    None
                } else {
                    Some(convert(scalar(0)?, work.budget)?)
                };
                flows.irr(guess, work.budget, work.cancelled)?
            }
            Self::MIRR => {
                let flows = CashFlows::new_with_input(element_count, element, work)?;
                let finance = convert(scalar(0)?, work.budget)?;
                let reinvest = convert(scalar(1)?, work.budget)?;
                flows.mirr(finance, reinvest, work.budget, work.cancelled)?
            }
        };

        finite(result).map(|v| Value::Double(Double::from_float(v)))
    }
}

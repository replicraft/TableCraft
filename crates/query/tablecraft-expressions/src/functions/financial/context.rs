//! Internal conversion/check bridge; numeric kernels retain a host-free default.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

pub(crate) trait FinancialContext {
    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError>;
    fn step(&mut self) -> Result<(), EvalError>;
}

pub(super) struct NumericContext<'a, F>(pub &'a mut F);
impl<F: FnMut() -> Result<(), EvalError>> FinancialContext for NumericContext<'_, F> {
    fn convert(&mut self, target: Conversion, value: &Value) -> Result<Value, EvalError> {
        target.apply(value)
    }
    fn step(&mut self) -> Result<(), EvalError> {
        self.0()
    }
}

use super::{GeneralCollation, adapt};
use std::cmp::Ordering;
use tablecraft_ace::{
    compare_general_like_units, general_like_literal_step, general_like_unit_step,
    is_general_like_unit,
};
use tablecraft_expressions::{EvalError, comparison::LikeSpanState};

impl GeneralCollation<'_> {
    pub(super) fn literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        general_like_literal_step(self.order, candidate, literal, &mut budget, self.cancelled)
            .map_err(adapt)
    }
    pub(super) fn unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        general_like_unit_step(self.order, candidate, &mut budget, self.cancelled).map_err(adapt)
    }
    pub(super) fn compare_pattern_units(
        &self,
        left: &[u16],
        right: &[u16],
    ) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_general_like_units(self.order, left, right, &mut budget, self.cancelled)
            .map_err(adapt)
    }
    pub(super) fn pattern_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        is_general_like_unit(self.order, units, &mut budget, self.cancelled).map_err(adapt)
    }
}

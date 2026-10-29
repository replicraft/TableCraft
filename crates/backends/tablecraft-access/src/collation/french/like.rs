//! French weighted LIKE retains a separate untrimmed key policy.
use super::{FRENCH_LEGACY_ORDER, FrenchLegacyCollation};
use crate::collation::adapt;
use std::cmp::Ordering;
use tablecraft_ace::{
    NativeSortOrder, compare_french_legacy_like_units, french_legacy_like_literal_step,
    french_legacy_like_unit_step, is_french_legacy_like_unit,
};
use tablecraft_expressions::{EvalError, comparison::LikeSpanState};

impl FrenchLegacyCollation<'_> {
    pub(super) fn literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        french_legacy_like_literal_step(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            candidate,
            literal,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }

    pub(super) fn unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        french_legacy_like_unit_step(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            candidate,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
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
        compare_french_legacy_like_units(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            left,
            right,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }

    pub(super) fn pattern_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        is_french_legacy_like_unit(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            units,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }
}

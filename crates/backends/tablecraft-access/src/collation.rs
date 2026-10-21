//! Explicit bridge from standalone General weights to supplied query comparison.
mod search;
use std::{cell::RefCell, cmp::Ordering};
use tablecraft_ace::{InspectError, NativeSortOrder, compare_general_text};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
};

/// Caller-selected provisional General comparison with one cumulative budget.
///
/// No database/field locale is inferred. This applies the standalone engine's
/// full-string weight model, not qualified native Access SQL semantics. Queries
/// must keep their original filters; this policy does not enable index pushdown.
/// The caller supplies cancellation and module/text-mode/LIKE policies separately.
/// Database-mode search uses the explicit full-weight consumed-span model.
/// No captured SQL, text values or sort keys enter tracing.
pub struct GeneralCollation<'a> {
    order: NativeSortOrder,
    budget: RefCell<ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> GeneralCollation<'a> {
    /// Validate the descriptor and retain the caller-issued operation budget.
    pub fn new(
        order: NativeSortOrder,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        order.general_family().ok_or(InspectError::Unsupported)?;
        Ok(Self {
            order,
            budget: RefCell::new(budget),
            cancelled,
        })
    }
}

impl Collation for GeneralCollation<'_> {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_general_text(self.order, left, right, &mut budget, self.cancelled).map_err(adapt)
    }
    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<std::ops::Range<usize>>, EvalError> {
        self.search(haystack, needle, search, mode)
    }
}

fn adapt(error: InspectError) -> EvalError {
    match error {
        InspectError::Limit(error) => EvalError::Limit(error),
        InspectError::Cancelled => EvalError::Cancelled,
        InspectError::Allocation => EvalError::Allocation,
        InspectError::StructureLimit => EvalError::TextLimit,
        _ => EvalError::CollationFailed,
    }
}

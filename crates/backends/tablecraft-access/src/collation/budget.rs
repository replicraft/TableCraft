//! Owned or explicitly borrowed comparison quota; no per-policy reset.
use std::cell::{BorrowMutError, RefCell, RefMut};
use tablecraft_core::limits::ResourceBudget;

pub(super) enum Budget<'a> {
    Owned(RefCell<ResourceBudget>),
    Shared(&'a RefCell<ResourceBudget>),
}
impl Budget<'_> {
    pub(super) fn try_borrow_mut(&self) -> Result<RefMut<'_, ResourceBudget>, BorrowMutError> {
        match self {
            Self::Owned(budget) => budget.try_borrow_mut(),
            Self::Shared(budget) => budget.try_borrow_mut(),
        }
    }
}

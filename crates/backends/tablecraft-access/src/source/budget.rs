//! One native allowance, owned by a source or borrowed from its explicit host.
use std::cell::{BorrowMutError, RefCell, RefMut};
use tablecraft_core::limits::ResourceBudget;

pub(super) enum Budget<'a> {
    Owned(RefCell<ResourceBudget>),
    Shared(&'a RefCell<ResourceBudget>),
}

impl Budget<'_> {
    pub(super) fn shared(&self) -> bool {
        matches!(self, Self::Shared(_))
    }

    pub(super) fn aliases(&self, other: &RefCell<ResourceBudget>) -> bool {
        matches!(self, Self::Shared(budget) if std::ptr::eq(*budget, other))
    }

    pub(super) fn try_borrow_mut(&self) -> Result<RefMut<'_, ResourceBudget>, BorrowMutError> {
        match self {
            Self::Owned(budget) => budget.try_borrow_mut(),
            Self::Shared(budget) => budget.try_borrow_mut(),
        }
    }
}

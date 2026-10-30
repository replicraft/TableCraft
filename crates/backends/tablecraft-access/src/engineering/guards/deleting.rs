use super::{AccessEngineeringDeleter, GuardBackendError, GuardPreparationSummary};
use tablecraft_ace::BeforeEventGuardSelector;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{DeletableTable, RowHandle, Snapshot};

pub(super) struct GuardDeleter<'a> {
    pub inner: AccessEngineeringDeleter,
    pub selectors: &'a [BeforeEventGuardSelector<'a>],
}

impl DeletableTable for GuardDeleter<'_> {
    type Error = GuardBackendError;
    type Plan = GuardPreparationSummary;

    fn snapshot(&self) -> &Snapshot {
        self.inner.snapshot()
    }

    fn plan_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .prepare_before_guards(self.selectors, rows, budget, cancelled)
    }
}

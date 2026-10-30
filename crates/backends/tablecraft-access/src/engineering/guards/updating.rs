use super::{AccessEngineeringEditor, GuardBackendError, GuardPreparationSummary};
use tablecraft_ace::BeforeEventGuardSelector;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, UpdateProfile};

pub(super) struct GuardEditor<'a> {
    pub inner: AccessEngineeringEditor,
    pub selectors: &'a [BeforeEventGuardSelector<'a>],
}

impl EditableTable for GuardEditor<'_> {
    type Error = GuardBackendError;
    type Plan = GuardPreparationSummary;

    fn snapshot(&self) -> &Snapshot {
        self.inner.snapshot()
    }

    fn update_profile(&self) -> UpdateProfile {
        self.inner.update_profile()
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .prepare_before_guards(self.selectors, column, updates, budget, cancelled)
    }
}

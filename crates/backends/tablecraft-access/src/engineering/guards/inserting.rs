use super::{AccessEngineeringInserter, GuardBackendError, GuardPreparationSummary};
use tablecraft_ace::BeforeEventGuardSelector;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::InsertableTable;
use tablecraft_schema::{Column, SchemaPolicy};

pub(super) struct GuardInserter<'a> {
    pub inner: AccessEngineeringInserter,
    pub selectors: &'a [BeforeEventGuardSelector<'a>],
}

impl InsertableTable for GuardInserter<'_> {
    type Error = GuardBackendError;
    type Plan = GuardPreparationSummary;

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn columns(&self) -> &[Column] {
        self.inner.columns()
    }

    fn schema_policy(&self) -> SchemaPolicy {
        self.inner.schema_policy()
    }

    fn requires_field_list(&self) -> bool {
        self.inner.requires_field_list()
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .prepare_before_guards(self.selectors, values, budget, cancelled)
    }
}

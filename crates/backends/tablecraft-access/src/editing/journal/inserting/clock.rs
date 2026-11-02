//! Actual query binder over metadata-only insertion and an explicit host provider.
use super::{Error, NativeJournalInsertPlan, NativeJournalInserter};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_schema::{Column, SchemaPolicy};

pub(in crate::editing) struct ClockInserter<'a> {
    inserter: NativeJournalInserter,
    provider: &'a dyn Clock,
}

impl<'a> ClockInserter<'a> {
    pub(super) fn new(inserter: NativeJournalInserter, provider: &'a dyn Clock) -> Self {
        Self { inserter, provider }
    }
}

impl InsertableTable for ClockInserter<'_> {
    type Error = Error;
    type Plan = NativeJournalInsertPlan;

    fn name(&self) -> &str {
        self.inserter.name()
    }
    fn columns(&self) -> &[Column] {
        self.inserter.columns()
    }
    fn schema_policy(&self) -> SchemaPolicy {
        self.inserter.schema_policy()
    }
    fn requires_field_list(&self) -> bool {
        self.inserter.requires_field_list()
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inserter.plan_insert(values, budget, cancelled)
    }
}

impl OmittingInsertableTable for ClockInserter<'_> {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inserter
            .plan_insert_with_omissions_and_clock(values, self.provider, budget, cancelled)
    }
}

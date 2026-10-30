//! Borrowed operation clock capability, without eager metadata evaluation.
use super::{AccessEngineeringInserter, EngineeringCopy, NativeEditError};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_schema::{Column, SchemaPolicy};

pub(in crate::engineering) struct ClockInserter<'a> {
    inner: AccessEngineeringInserter,
    clock: &'a dyn Clock,
}

impl<'a> ClockInserter<'a> {
    pub(in crate::engineering) fn new(
        inner: AccessEngineeringInserter,
        clock: &'a dyn Clock,
    ) -> Self {
        Self { inner, clock }
    }
}

impl InsertableTable for ClockInserter<'_> {
    type Error = NativeEditError;
    type Plan = EngineeringCopy;

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
        self.inner.plan_insert(values, budget, cancelled)
    }
}

impl OmittingInsertableTable for ClockInserter<'_> {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .plan_presence(values, Some(self.clock), budget, cancelled)
    }
}

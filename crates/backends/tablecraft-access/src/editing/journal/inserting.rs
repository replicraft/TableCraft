//! Metadata-only insertion; complete native admission occurs at finish.
mod batch;
mod capture;
pub(in crate::editing) mod clock;
mod complete_batch;
mod plan;
mod presence;

use super::Error;
use crate::editing::NativeEditError;
pub use plan::NativeJournalInsertPlan;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::RecoverableEditError;
use tablecraft_ace::{InspectError, NativeImage, RowInsert, inspect_table_schema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_schema::{Column, NameComparison, SchemaPolicy};

const MAX_NAME_BYTES: usize = 255;
const MAX_COLUMNS: usize = 256;

/// A complete insertion schema, without a target-row snapshot or resolved values.
/// This capability retains the immutable image; it grants no mutation permission.
pub struct NativeJournalInserter {
    image: NativeImage,
    name: String,
    columns: Vec<Column>,
}

impl NativeJournalInserter {
    /// Inspect the actual complete schema without scanning target records.
    pub fn new(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        let _span = tracing::debug_span!("access.insert.capture_metadata").entered();
        check(budget, cancelled)?;
        let schema = inspect_table_schema(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            table,
            budget,
            cancelled,
        )?;
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let work = column
                .name()
                .len()
                .checked_mul(position + 1)
                .ok_or(InspectError::StructureLimit)?;
            budget
                .charge(Resource::Steps, work as u64)
                .map_err(InspectError::from)?;
        }
        let columns = schema.scalar_columns_with_budget(budget)?;
        budget
            .charge_allocation(schema.name().len() as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, schema.name().len() as u64)
            .map_err(InspectError::from)?;
        let mut name = String::new();
        name.try_reserve_exact(schema.name().len())
            .map_err(|_| InspectError::Allocation)?;
        name.push_str(schema.name());
        check(budget, cancelled)?;
        Ok(Self {
            image,
            name,
            columns,
        })
    }
}

impl InsertableTable for NativeJournalInserter {
    type Error = Error;
    type Plan = NativeJournalInsertPlan;

    fn name(&self) -> &str {
        &self.name
    }

    fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn schema_policy(&self) -> SchemaPolicy {
        SchemaPolicy {
            max_name_bytes: MAX_NAME_BYTES,
            max_columns: MAX_COLUMNS,
            names: NameComparison::AsciiInsensitive,
        }
    }

    fn requires_field_list(&self) -> bool {
        false
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        check(budget, cancelled)?;
        #[cfg(feature = "ace16-create")]
        if self
            .image
            .select_native_retained_keyless_insert_table(&self.name, budget, cancelled)
            .map_err(RecoverableEditError::from)?
        {
            return self.plan_supplied_presence(values, budget, cancelled);
        }
        budget
            .charge_allocation(std::mem::size_of::<RowInsert>() as u64, 1)
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(1)
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        rows.push(RowInsert::new(values));
        self.image
            .plan_recoverable_ordinary_complete_row_inserts(&self.name, rows, budget, cancelled)
            .map(NativeJournalInsertPlan::CompleteTyped)
            .map_err(Error::from)
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), NativeEditError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

impl OmittingInsertableTable for NativeJournalInserter {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.plan_presence(values, None, budget, cancelled)
    }
}

impl NativeJournalInserter {
    /// Explicit host clock capability; metadata construction does not sample it.
    pub fn plan_insert_with_omissions_and_clock(
        self,
        values: Vec<InsertValue>,
        provider: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeJournalInsertPlan, Error> {
        self.plan_presence(values, Some(provider), budget, cancelled)
    }

    pub(in crate::editing) fn with_clock(self, provider: &dyn Clock) -> clock::ClockInserter<'_> {
        clock::ClockInserter::new(self, provider)
    }
}

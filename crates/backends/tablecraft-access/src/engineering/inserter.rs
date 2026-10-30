//! Explicit SQL insertion target consuming one existing engineering preview.
mod omitting;
use super::{EngineeringCopy, EngineeringRequest, preview};
use crate::editing::NativeEditError;
use tablecraft_ace::{
    AutoNumberRowInsert, IncrementingAutoNumber, InspectError, NativeImage, RowInsert,
    inspect_table_schema,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertableTable;
use tablecraft_schema::{Column, NameComparison, SchemaPolicy};

/// Explicit row-generation selection; absence never infers a native mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarInsertMode {
    /// Ordinary insertion; generated fields remain refused. The complete
    /// capability requires every field, while the separate omission capability
    /// may preserve missing ordinary fields for engine resolution.
    Explicit,
    /// Select increment-by-one behavior and omit only the engine-bound primary.
    Incrementing,
}

/// One immutable image with a complete supplied-column metadata contract.
/// Columns come from native declarations without a row snapshot. Explicit
/// generation additionally runs its engine source qualification before binding;
/// query expression binding itself performs no source scans.
pub struct AccessEngineeringInserter {
    image: NativeImage,
    name: String,
    columns: Vec<Column>,
    policy: Option<IncrementingAutoNumber>,
}

impl AccessEngineeringInserter {
    /// Resolve metadata and explicit generation policy from the same image.
    /// Selecting generation performs the existing complete before-INSERT checks.
    /// Generated-column detection stays inside ACE; no flags/offsets are read here.
    pub fn new(
        image: NativeImage,
        table: &str,
        mode: ScalarInsertMode,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
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
        let mut columns = schema.scalar_columns_with_budget(budget)?;

        let policy = match mode {
            ScalarInsertMode::Explicit => None,
            ScalarInsertMode::Incrementing => {
                let binding =
                    image.prepare_ace16_incrementing_insert_binding(table, budget, cancelled)?;
                let position = binding.generated_position();
                let column = columns.get(position).ok_or(InspectError::Malformed)?;
                if column.name() != binding.generated_name() {
                    return Err(InspectError::Malformed.into());
                }
                budget
                    .charge(Resource::Steps, columns.len() as u64)
                    .map_err(InspectError::from)?;
                columns.remove(position);
                Some(binding.policy())
            }
        };

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
            policy,
        })
    }
}

impl InsertableTable for AccessEngineeringInserter {
    type Error = NativeEditError;
    type Plan = EngineeringCopy;

    fn name(&self) -> &str {
        &self.name
    }

    fn columns(&self) -> &[Column] {
        &self.columns
    }

    fn schema_policy(&self) -> SchemaPolicy {
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        }
    }

    fn requires_field_list(&self) -> bool {
        self.policy.is_some()
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;

        let copy = if let Some(policy) = self.policy {
            let rows = [AutoNumberRowInsert::new(values)];

            preview(
                self.image,
                EngineeringRequest::IncrementingInsert {
                    table: &self.name,
                    rows: &rows,
                    policy,
                },
                budget,
                cancelled,
            )
        } else {
            let rows = [RowInsert::new(values)];

            preview(
                self.image,
                EngineeringRequest::Insert {
                    table: &self.name,
                    rows: &rows,
                },
                budget,
                cancelled,
            )
        };

        copy.map_err(|error| match error {
            super::EngineeringError::Engine(error) => NativeEditError::Edit(error),
        })
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

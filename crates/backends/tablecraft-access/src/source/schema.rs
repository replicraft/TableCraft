use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, TableSchema, inspect_table_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_schema::{Column, SchemaName};

pub(super) struct ScalarSchema {
    pub columns: Vec<Column>,
    pub unavailable: Vec<SchemaName>,
}

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn schema_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let entry = self.entry(table)?;
        tracing::debug!(
            cached = entry.columns.get().is_some(),
            "native schema access"
        );
        if entry.columns.get().is_none() {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let schema = inspect_table_schema(
                &mut *source,
                self.scope,
                &entry.name,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            let columns = classify(&schema, &mut budget, self.cancelled).map_err(query_error)?;
            entry.columns.set(columns).map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.columns.get().ok_or(QueryError::Source)?.columns)
    }
}

fn classify(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ScalarSchema, InspectError> {
    let count = schema.columns().len();
    budget.charge_allocation(count as u64, size_of::<Column>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<SchemaName>() as u64)?;
    let mut columns = Vec::new();
    let mut unavailable = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    unavailable
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for field in schema.columns() {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        budget.charge_allocation(field.name().len() as u64, 1)?;
        if field.scalar_type().is_some() {
            columns.push(field.column()?);
        } else {
            let mut name = String::new();
            name.try_reserve_exact(field.name().len())
                .map_err(|_| InspectError::Allocation)?;
            name.push_str(field.name());
            unavailable.push(SchemaName::new(name)?);
        }
    }
    Ok(ScalarSchema {
        columns,
        unavailable,
    })
}

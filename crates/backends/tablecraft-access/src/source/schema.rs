use super::{NativeSource, query_error};
use tablecraft_ace::{InspectError, TableSchema, inspect_table_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_schema::{Column, SchemaName};

pub(super) struct ScalarSchema {
    pub columns: Vec<Column>,
    pub unavailable: Vec<SchemaName>,
}

impl ScalarSchema {
    pub fn projection_names(&self, budget: &mut ResourceBudget) -> Result<Vec<&str>, InspectError> {
        budget.charge_allocation(self.columns.len() as u64, size_of::<&str>() as u64)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(self.columns.len())
            .map_err(|_| InspectError::Allocation)?;
        names.extend(self.columns.iter().map(|column| column.name()));
        Ok(names)
    }
    pub fn selected_names(
        &self,
        indices: &[usize],
        budget: &mut ResourceBudget,
    ) -> Result<Vec<&str>, InspectError> {
        budget.charge_allocation(indices.len() as u64, size_of::<&str>() as u64)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(indices.len())
            .map_err(|_| InspectError::Allocation)?;
        for index in indices {
            names.push(
                self.columns
                    .get(*index)
                    .ok_or(InspectError::Malformed)?
                    .name(),
            );
        }
        Ok(names)
    }
    pub fn matches_projection(
        &self,
        snapshot: &tablecraft_ace::TableSnapshot,
        indices: &[usize],
    ) -> bool {
        if snapshot.columns().len() != indices.len()
            || !snapshot
                .columns()
                .iter()
                .zip(indices)
                .all(|(column, index)| self.columns.get(*index) == Some(column))
        {
            return false;
        }
        let mut columns = self.columns.iter();
        let mut unavailable = self.unavailable.iter();
        for field in snapshot.schema().columns() {
            if let Some(kind) = field.scalar_type() {
                let Some(column) = columns.next() else {
                    return false;
                };
                if column.name() != field.name()
                    || column.data_type() != kind
                    || column.nullable() != (kind != tablecraft_schema::ScalarType::Boolean)
                {
                    return false;
                }
            } else if unavailable.next().map(|name| name.as_str()) != Some(field.name()) {
                return false;
            }
        }
        columns.next().is_none() && unavailable.next().is_none()
    }
    pub fn matches(&self, snapshot: &tablecraft_ace::TableSnapshot) -> bool {
        self.columns == snapshot.columns()
            && snapshot
                .schema()
                .columns()
                .iter()
                .filter(|field| field.scalar_type().is_none())
                .map(|field| field.name())
                .eq(self.unavailable.iter().map(|name| name.as_str()))
    }
}

impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
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
            super::collation::validate(&schema, self.database_order, &mut budget, self.cancelled)
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

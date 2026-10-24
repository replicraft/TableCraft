use super::{NativeSource, collation::text_collation, query_error};
use tablecraft_ace::{
    InspectError, NativeColumn, TableSchema, inspect_table_features, inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{QueryError, source::TextCollationId};
use tablecraft_schema::{Column, SchemaName};

pub(super) struct ScalarSchema {
    pub columns: Vec<Column>,
    pub unavailable: Vec<SchemaName>,
    pub text_collations: Vec<Option<TextCollationId>>,
    pub cached: Vec<Option<StoredShape>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct StoredShape {
    native_type: u8,
    flags: u8,
    extended_flags: u8,
    bytes: usize,
    number: usize,
    fixed: bool,
    decimal: Option<(u8, u8)>,
}
impl StoredShape {
    fn of(field: &NativeColumn) -> Self {
        Self {
            native_type: field.native_type(),
            flags: field.flags(),
            extended_flags: field.extended_flags(),
            bytes: field.storage_length(),
            number: field.number(),
            fixed: field.fixed(),
            decimal: field.decimal_parameters(),
        }
    }
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
        self.matches_native(snapshot.schema())
    }
    pub fn matches(&self, snapshot: &tablecraft_ace::TableSnapshot) -> bool {
        self.columns == snapshot.columns() && self.matches_native(snapshot.schema())
    }
    pub fn has_cached(&self, indices: Option<&[usize]>) -> bool {
        match indices {
            Some(indices) => indices
                .iter()
                .any(|index| self.cached.get(*index).is_some_and(Option::is_some)),
            None => self.cached.iter().any(Option::is_some),
        }
    }
    fn matches_native(&self, schema: &TableSchema) -> bool {
        let mut available = self.columns.iter().zip(&self.cached).peekable();
        let mut unavailable = self.unavailable.iter();
        let mut policies = self.text_collations.iter();
        for field in schema.columns() {
            if available
                .peek()
                .is_some_and(|(column, _)| column.name() == field.name())
            {
                let Some((column, cached)) = available.next() else {
                    return false;
                };
                let valid = match cached {
                    Some(shape) => *shape == StoredShape::of(field) && column.nullable(),
                    None => {
                        field.scalar_type() == Some(column.data_type())
                            && column.nullable()
                                == (column.data_type() != tablecraft_schema::ScalarType::Boolean)
                    }
                };
                if !valid || policies.next().copied() != Some(text_collation(field)) {
                    return false;
                }
            } else if unavailable.next().map(|name| name.as_str()) != Some(field.name()) {
                return false;
            }
        }
        available.next().is_none() && unavailable.next().is_none() && policies.next().is_none()
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
            let columns =
                if self.stored_calculations {
                    let features = inspect_table_features(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?;
                    super::collation::validate(
                        features.schema(),
                        self.text_policy_scope.required_order(),
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?;
                    let mut columns = empty(features.schema().columns().len(), &mut budget)
                        .map_err(query_error)?;
                    for field in features.columns() {
                        if (self.cancelled)() {
                            return Err(QueryError::Cancelled);
                        }
                        budget.charge(Resource::Steps, 1)?;
                        match field.stored_column(&mut budget) {
                            Ok(column) => {
                                if column.data_type() == tablecraft_schema::ScalarType::Text
                                    && self.text_policy_scope.required_order().is_some_and(
                                        |order| field.column().text_sort_order() != Some(order),
                                    )
                                {
                                    return Err(QueryError::Unsupported);
                                }
                                columns.cached.push(
                                    field.calculated().then(|| StoredShape::of(field.column())),
                                );
                                columns.text_collations.push(text_collation(field.column()));
                                columns.columns.push(column);
                            }
                            Err(InspectError::Unsupported) => {
                                budget
                                    .charge_allocation(field.column().name().len() as u64, 1)
                                    .map_err(|error| query_error(error.into()))?;
                                let mut name = String::new();
                                name.try_reserve_exact(field.column().name().len())
                                    .map_err(|_| QueryError::Allocation)?;
                                name.push_str(field.column().name());
                                columns.unavailable.push(
                                    SchemaName::new(name)
                                        .map_err(|error| query_error(error.into()))?,
                                );
                            }
                            Err(error) => return Err(query_error(error)),
                        }
                    }
                    columns
                } else {
                    let schema = inspect_table_schema(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?;
                    super::collation::validate(
                        &schema,
                        self.text_policy_scope.required_order(),
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?;
                    classify(&schema, &mut budget, self.cancelled).map_err(query_error)?
                };
            entry.columns.set(columns).map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.columns.get().ok_or(QueryError::Source)?.columns)
    }
}

fn empty(count: usize, budget: &mut ResourceBudget) -> Result<ScalarSchema, InspectError> {
    budget.charge_allocation(count as u64, size_of::<Column>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<SchemaName>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<Option<TextCollationId>>() as u64)?;
    budget.charge_allocation(count as u64, size_of::<Option<StoredShape>>() as u64)?;
    let mut cached = Vec::new();
    cached
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut text_collations = Vec::new();
    text_collations
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut columns = Vec::new();
    let mut unavailable = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    unavailable
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(ScalarSchema {
        text_collations,
        cached,
        columns,
        unavailable,
    })
}

fn classify(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ScalarSchema, InspectError> {
    let mut output = empty(schema.columns().len(), budget)?;
    for field in schema.columns() {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        budget.charge_allocation(field.name().len() as u64, 1)?;
        if field.scalar_type().is_some() {
            output.columns.push(field.column()?);
            output.cached.push(None);
            output.text_collations.push(text_collation(field));
        } else {
            let mut name = String::new();
            name.try_reserve_exact(field.name().len())
                .map_err(|_| InspectError::Allocation)?;
            name.push_str(field.name());
            output.unavailable.push(SchemaName::new(name)?);
        }
    }
    Ok(output)
}

use super::super::{
    CreateColumn, CreateDatabaseError,
    column::Declaration,
    input::{Columns, Rows},
    request,
};
use crate::{InspectError, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

struct Column {
    name: String,
    description: Option<String>,
    declaration: Declaration,
    default: Option<Value>,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
}

/// Owned original declarations for the first table in the canonical blank file.
/// This is request data, without persistence or imported-file mutation authority.
pub struct FirstTableDefinition {
    table: String,
    primary: String,
    columns: Vec<Column>,
}

fn copy_name(name: &str, budget: &mut ResourceBudget) -> Result<String, InspectError> {
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, name.len() as u64)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(owned)
}

impl FirstTableDefinition {
    /// Validate and retain exact names and empty Increment declarations.
    /// The first field must be an Int32 Increment primary. No initial rows,
    /// generated-field policies/defaults, physical offsets or native flags are
    /// accepted. Ordinary companion fields retain checked constant defaults and
    /// exact optional Required/AllowZeroLength declarations.
    pub fn capture(
        table: &str,
        columns: &[CreateColumn<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        let _span = tracing::debug_span!("native.first_table.capture_definition").entered();
        Self::validate_borrowed(table, columns, budget, cancelled)?;
        let primary = columns[0]
            .primary_key_name()
            .ok_or(CreateDatabaseError::Declaration)?;
        let table = copy_name(table, budget)?;
        let primary = copy_name(primary, budget)?;
        budget
            .charge_allocation(columns.len() as u64, size_of::<Column>() as u64)
            .map_err(InspectError::from)?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for column in columns {
            check(budget, cancelled)?;
            retained.push(Column {
                name: copy_name(column.name(), budget)?,
                description: column
                    .description()
                    .map(|text| copy_name(text, budget))
                    .transpose()?,
                declaration: column.declaration(),
                required: column.required(),
                allow_zero_length: column.allow_zero_length(),
                default: column
                    .default_literal()
                    .map(|value| copy_default(value, budget))
                    .transpose()?,
            });
        }
        check(budget, cancelled)?;
        Ok(Self {
            table,
            primary,
            columns: retained,
        })
    }

    fn validate_borrowed(
        table: &str,
        columns: &[CreateColumn<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        if !columns
            .first()
            .is_some_and(|column| column.incrementing_autonumber())
        {
            return Err(CreateDatabaseError::Declaration);
        }
        request::validate(
            table,
            Columns::Values(columns),
            Rows::Values(&[]),
            budget,
            cancelled,
        )?;
        Ok(())
    }

    /// Exact proposed native table name.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact explicit primary-index name.
    pub fn primary_name(&self) -> &str {
        &self.primary
    }

    /// Number of original ordered declarations.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub(crate) fn borrowed_columns(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<CreateColumn<'_>>, CreateDatabaseError> {
        check(budget, cancelled)?;
        budget
            .charge_allocation(
                self.columns.len() as u64,
                size_of::<CreateColumn<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for (position, column) in self.columns.iter().enumerate() {
            check(budget, cancelled)?;
            let borrowed = match column.declaration {
                Declaration::Fixed(kind) => CreateColumn::new(&column.name, kind),
                Declaration::ShortText { units } => CreateColumn::short_text(&column.name, units)?,
                Declaration::ShortBinary { bytes } => {
                    CreateColumn::short_binary(&column.name, bytes)?
                }
                Declaration::Decimal(constraints) => {
                    CreateColumn::decimal(&column.name, constraints)
                }
            };
            let borrowed = if position == 0 {
                borrowed
                    .with_primary_key(&self.primary)?
                    .with_incrementing_autonumber()?
            } else {
                borrowed
            };
            let borrowed = match column.required {
                Some(required) => borrowed.with_required(required),
                None => borrowed,
            };
            let borrowed = match column.allow_zero_length {
                Some(allow) => borrowed.with_allow_zero_length(allow)?,
                None => borrowed,
            };
            let borrowed = match &column.default {
                Some(value) => borrowed.with_default_literal(value)?,
                None => borrowed,
            };
            let borrowed = match &column.description {
                Some(description) => borrowed.with_description(description)?,
                None => borrowed,
            };
            columns.push(borrowed);
        }
        check(budget, cancelled)?;
        Ok(columns)
    }
}

fn copy_default(value: &Value, budget: &mut ResourceBudget) -> Result<Value, InspectError> {
    Ok(match value {
        Value::Text(text) => Value::Text(copy_name(text, budget)?),
        Value::Null => Value::Null,
        Value::Byte(value) => Value::Byte(*value),
        Value::Int16(value) => Value::Int16(*value),
        Value::Single(value) => Value::Single(*value),
        Value::Double(value) => Value::Double(*value),
        Value::Decimal(value) => Value::Decimal(*value),
        Value::Boolean(value) => Value::Boolean(*value),
        Value::Int32(value) => Value::Int32(*value),
        Value::Int64(value) => Value::Int64(*value),
        Value::Currency(value) => Value::Currency(*value),
        Value::DateTime(value) => Value::DateTime(*value),
        _ => unreachable!("validated constant-default family"),
    })
}

//! Query descriptors remain distinct from native destination storage declarations.
use super::{MakeTableError, check};
use crate::{OutputType, QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::{ScalarType, SchemaError, SchemaName, TypeDescriptor};

/// One named output's established query descriptor, not a native CREATE COLUMN.
/// Text length, when present, counts query Unicode scalars rather than native
/// UTF-16 units. No primary key, generated value, default, Required, index or other
/// source field/table property is copied or invented by this declaration.
pub struct MakeTableColumn {
    name: SchemaName,
    data_type: TypeDescriptor,
    output_nullable: bool,
}

impl MakeTableColumn {
    /// Original output name, including explicit aliases.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Established query value carrier and its represented constraints.
    /// Native physical type/field-size resolution remains the publisher's work.
    pub const fn data_type(&self) -> TypeDescriptor {
        self.data_type
    }

    /// Whether this query output may contain NULL. This is not a destination
    /// Required property and must not be used to inherit NOT NULL from a source.
    pub const fn output_nullable(&self) -> bool {
        self.output_nullable
    }
}

pub(super) fn columns(
    names: Vec<String>,
    types: &[OutputType],
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Vec<MakeTableColumn>, MakeTableError> {
    if names.is_empty() || names.len() != types.len() {
        return Err(QueryError::Width.into());
    }
    budget
        .charge_allocation(names.len() as u64, size_of::<MakeTableColumn>() as u64)
        .map_err(QueryError::from)?;
    let mut columns: Vec<MakeTableColumn> = Vec::new();
    columns
        .try_reserve_exact(names.len())
        .map_err(|_| QueryError::Allocation)?;
    for (name, declaration) in names.into_iter().zip(types) {
        check(budget, cancelled)?;
        for previous in &columns {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    previous.name().len().max(name.len()) as u64,
                )
                .map_err(QueryError::from)?;
            if SOURCE_SCHEMA_POLICY.names.equals(previous.name(), &name) {
                return Err(QueryError::Schema(SchemaError::Duplicate).into());
            }
        }
        let (data_type, output_nullable) = match *declaration {
            OutputType::Scalar {
                data_type,
                nullable,
            } => (data_type, nullable),
            OutputType::Null | OutputType::Dynamic => {
                return Err(MakeTableError::IndeterminateType);
            }
            OutputType::Multivalue { .. } | OutputType::Recordset { .. } => {
                return Err(MakeTableError::StructuredOutput);
            }
        };
        columns.push(MakeTableColumn {
            name: SchemaName::new(name).map_err(QueryError::from)?,
            data_type,
            output_nullable,
        });
    }
    Ok(columns)
}

pub(super) fn validate(
    column: &MakeTableColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    check(budget, cancelled)?;
    if matches!(value, Value::Null) {
        return if column.output_nullable {
            Ok(())
        } else {
            Err(SchemaError::Value.into())
        };
    }
    if ScalarType::of(value) != Some(column.data_type.scalar_type()) {
        return Err(SchemaError::Value.into());
    }
    if let Value::Text(text) = value
        && let Some(maximum) = column.data_type.max_text_characters()
    {
        for (position, _) in text.chars().enumerate() {
            check(budget, cancelled)?;
            if position >= maximum {
                return Err(SchemaError::Value.into());
            }
        }
    }
    if let Value::Decimal(decimal) = value
        && let Some(constraints) = column.data_type.decimal_constraints()
    {
        constraints.validate(*decimal)?;
    }
    Ok(())
}

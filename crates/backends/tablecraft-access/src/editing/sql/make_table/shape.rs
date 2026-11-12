//! Closed syntax and native field-size capture; no row inference or property copying.
mod computed;
use super::NativeMakeTableError;
pub(super) use computed::{preflight, staged_columns};
use tablecraft_ace::{CreateColumn, InspectError, NativeColumn, TableSchema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{DecimalConstraints, NameComparison, ScalarType};
use tablecraft_sql::{
    query::{Query, Statement},
    select::{FieldReference, Projection, Select, Table, TableSource},
};

pub(super) fn check(
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    work.charge(Resource::Steps, 1)?;
    Ok(())
}

pub(super) fn charge_name(
    left: &str,
    right: &str,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(work, cancelled)?;
    work.charge(Resource::Steps, left.len().max(right.len()) as u64)?;
    Ok(())
}

pub(super) fn select(
    query: &Query,
    declared_parameters: bool,
) -> Result<&Select, NativeMakeTableError> {
    let Statement::MakeTable(select) = &query.statement else {
        return Err(NativeMakeTableError::Shape);
    };
    if (!declared_parameters && !query.parameters.is_empty())
        || select.into_external.is_some()
        || select.external.is_some()
        || select.owner_access
        || select.projection.is_empty()
    {
        return Err(NativeMakeTableError::Shape);
    }
    table(select)?;
    Ok(select)
}

pub(super) fn table(select: &Select) -> Result<&Table, NativeMakeTableError> {
    let [TableSource::Table(table)] = select.from.as_slice() else {
        return Err(NativeMakeTableError::Shape);
    };
    if table.database.is_some() {
        return Err(NativeMakeTableError::Shape);
    }
    Ok(table)
}

fn resolve<'a>(
    reference: &FieldReference,
    table: &Table,
    schema: &'a TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a NativeColumn, NativeMakeTableError> {
    resolve_optional(reference, table, schema, work, cancelled)?.ok_or(NativeMakeTableError::Shape)
}

fn resolve_optional<'a>(
    reference: &FieldReference,
    table: &Table,
    schema: &'a TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<&'a NativeColumn>, NativeMakeTableError> {
    if let Some(qualifier) = &reference.qualifier {
        let name = table.alias.as_ref().unwrap_or(&table.name);
        charge_name(qualifier.as_str(), name.as_str(), work, cancelled)?;
        if !NameComparison::AsciiInsensitive.equals(qualifier.as_str(), name.as_str()) {
            return Err(NativeMakeTableError::Shape);
        }
    }
    for column in schema.columns() {
        charge_name(column.name(), reference.name.as_str(), work, cancelled)?;
        if NameComparison::AsciiInsensitive.equals(column.name(), reference.name.as_str()) {
            return Ok(Some(column));
        }
    }
    Ok(None)
}

pub(super) fn columns<'a>(
    select: &'a Select,
    schema: &TableSchema,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<CreateColumn<'a>>, NativeMakeTableError> {
    let table = table(select)?;
    work.charge_allocation(
        select.projection.len() as u64,
        size_of::<CreateColumn<'_>>() as u64,
    )
    .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(select.projection.len())
        .map_err(|_| InspectError::Allocation)?;
    for projection in &select.projection {
        check(work, cancelled)?;
        let Projection::Field { field, alias } = projection else {
            return Err(NativeMakeTableError::Shape);
        };
        let source = resolve(field, table, schema, work, cancelled)?;
        let name = alias.as_ref().unwrap_or(&field.name).as_str();
        let column = native_column(name, source)?;
        columns.push(column);
    }
    Ok(columns)
}

fn native_column<'a>(
    name: &'a str,
    source: &NativeColumn,
) -> Result<CreateColumn<'a>, NativeMakeTableError> {
    let scalar = source.scalar_type().ok_or(NativeMakeTableError::Shape)?;
    let column = match scalar {
        ScalarType::Text if source.is_ordinary_short_text() => {
            CreateColumn::short_text(name, source.storage_length() / size_of::<u16>())?
        }
        ScalarType::Decimal => {
            let (precision, scale) = source
                .decimal_parameters()
                .ok_or(NativeMakeTableError::Shape)?;
            let constraints = DecimalConstraints::new(precision, scale)
                .map_err(|_| NativeMakeTableError::Shape)?;
            CreateColumn::decimal(name, constraints)
        }
        ScalarType::Boolean
        | ScalarType::Byte
        | ScalarType::Int16
        | ScalarType::Int32
        | ScalarType::Int64
        | ScalarType::Currency
        | ScalarType::Single
        | ScalarType::Double
        | ScalarType::DateTime
        | ScalarType::Guid => CreateColumn::new(name, scalar),
        _ => return Err(NativeMakeTableError::Shape),
    };
    Ok(column)
}

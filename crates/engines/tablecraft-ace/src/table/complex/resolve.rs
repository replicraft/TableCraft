use super::{ComplexColumnSchema, ComplexTableSchema};
use crate::{
    CatalogInspection, InspectError, TableSchema,
    format::{
        data_type,
        layout::{AUTO_NUMBER_FLAG, COMPLEX_FK_EXT_FLAG},
    },
    header::check,
    inspect_catalog, inspect_table_schema,
    table::read_selected,
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SYSTEM_TABLE: &str = "MSysComplexColumns";
const FIELDS: [&str; 5] = [
    "ComplexID",
    "ConceptualTableID",
    "ColumnName",
    "FlatTableID",
    "ComplexTypeObjectID",
];
const LOCAL_TABLE: i16 = 1;
const CHILD_SYSTEM_COLUMNS: usize = 2;
const CATALOG_LOOKUPS_PER_COLUMN: u64 = 2;

/// Resolve bounded complex-field metadata on a stable/offline source.
/// Rejects missing/duplicate/foreign-owner mappings and inconsistent child shapes.
/// No complex values, calculations, scripts or attachment content are executed.
pub fn inspect_complex_schema<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ComplexTableSchema, InspectError> {
    let _span = tracing::debug_span!("native.complex.schema").entered();
    let owner = inspect_table_schema(source, scope, name, budget, cancelled)?;
    let count = owner
        .columns()
        .iter()
        .filter(|c| c.complex_type_id().is_some())
        .count();
    if count == 0 {
        return Ok(ComplexTableSchema {
            owner,
            columns: Vec::new(),
        });
    }
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let owner_id = catalog
        .origins()
        .iter()
        .find(|o| o.object_id() == owner.object_id())
        .ok_or(InspectError::Malformed)?
        .native_id();
    let rows = read_selected(
        source,
        scope,
        SYSTEM_TABLE,
        Some(&FIELDS),
        budget,
        cancelled,
    )?;
    let mut positions = [0; FIELDS.len()];
    for (i, field) in FIELDS.iter().enumerate() {
        positions[i] = rows
            .columns()
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(field))
            .ok_or(InspectError::Malformed)?;
    }
    budget.charge_allocation(count as u64, size_of::<ComplexColumnSchema>() as u64)?;
    let mut columns: Vec<ComplexColumnSchema> = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for column in owner.columns() {
        check(budget, cancelled)?;
        let Some(type_id) = column.complex_type_id() else {
            continue;
        };
        if type_id == 0
            || type_id > i32::MAX as u32
            || !column.fixed()
            || column.storage_length() != data_type::INT32_BYTES
            || columns.iter().any(|c| c.type_id == type_id)
        {
            return Err(InspectError::Malformed);
        }
        let mut matched = None;
        for row in rows.rows() {
            check(budget, cancelled)?;
            if row.get(positions[0]) != Some(&Value::Int32(type_id as i32)) {
                continue;
            }
            if matched.is_some()
                || row.get(positions[1]) != Some(&Value::Int32(owner_id))
                || !matches!(row.get(positions[2]), Some(Value::Text(name)) if name.eq_ignore_ascii_case(column.name()))
            {
                return Err(InspectError::Malformed);
            }
            matched = Some(row);
        }
        let row = matched.ok_or(InspectError::Malformed)?;
        let native_id = |position| match row.get(position) {
            Some(Value::Int32(id)) if *id > 0 && *id != owner_id => Ok(*id),
            _ => Err(InspectError::Malformed),
        };
        let flat_id = native_id(positions[3])?;
        let type_table_id = native_id(positions[4])?;
        if flat_id == type_table_id
            || columns
                .iter()
                .any(|c| c.flat_table.definition_page() == flat_id as u32)
        {
            return Err(InspectError::Malformed);
        }
        let lookup_steps = (catalog.origins().len() + catalog.catalog().objects().len()) as u64;
        budget.charge(Resource::Steps, lookup_steps * CATALOG_LOOKUPS_PER_COLUMN)?;
        let type_table = inspect_table_schema(
            source,
            scope,
            table_name(&catalog, type_table_id)?,
            budget,
            cancelled,
        )?;
        let flat_table = inspect_table_schema(
            source,
            scope,
            table_name(&catalog, flat_id)?,
            budget,
            cancelled,
        )?;
        if flat_table.definition_page() == owner.definition_page()
            || type_table.definition_page() == owner.definition_page()
            || flat_table.definition_page() == type_table.definition_page()
            || columns
                .iter()
                .any(|c| c.flat_table.definition_page() == flat_table.definition_page())
        {
            return Err(InspectError::Malformed);
        }
        let (foreign_key, identity) = validate_shape(&type_table, &flat_table, budget, cancelled)?;
        columns.push(ComplexColumnSchema {
            number: column.number(),
            type_id,
            type_table,
            flat_table,
            foreign_key,
            identity,
        });
    }
    check(budget, cancelled)?;
    Ok(ComplexTableSchema { owner, columns })
}

fn table_name(catalog: &CatalogInspection, id: i32) -> Result<&str, InspectError> {
    let mut origins = catalog.origins().iter().filter(|o| o.native_id() == id);
    let origin = origins.next().ok_or(InspectError::Malformed)?;
    if origins.next().is_some() || origin.type_code() != LOCAL_TABLE {
        return Err(InspectError::Malformed);
    }
    Ok(catalog
        .catalog()
        .objects()
        .iter()
        .find(|o| o.id() == origin.object_id())
        .ok_or(InspectError::Malformed)?
        .name()
        .as_str())
}

fn validate_shape(
    template: &TableSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(usize, usize), InspectError> {
    if flat.columns().len() != template.columns().len() + CHILD_SYSTEM_COLUMNS {
        return Err(InspectError::Unsupported);
    }
    let mut foreign_key = None;
    let mut identity = None;
    for column in flat.columns() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, template.columns().len() as u64)?;
        if let Some(value) = template
            .columns()
            .iter()
            .find(|c| c.name().eq_ignore_ascii_case(column.name()))
        {
            if value.native_type() != column.native_type()
                || !super::value_schema::same_length(value, column, template.declared_rows())
            {
                return Err(InspectError::Malformed);
            }
            continue;
        }
        if column.native_type() != data_type::INT32
            || !column.fixed()
            || column.storage_length() != data_type::INT32_BYTES
        {
            return Err(InspectError::Unsupported);
        }
        if column.extended_flags() == COMPLEX_FK_EXT_FLAG
            && column.flags() & AUTO_NUMBER_FLAG == 0
            && foreign_key.is_none()
        {
            foreign_key = Some(column.number());
        } else if column.extended_flags() == 0
            && column.flags() & AUTO_NUMBER_FLAG != 0
            && identity.is_none()
        {
            identity = Some(column.number());
        } else {
            return Err(InspectError::Unsupported);
        }
    }
    Ok((
        foreign_key.ok_or(InspectError::Malformed)?,
        identity.ok_or(InspectError::Malformed)?,
    ))
}

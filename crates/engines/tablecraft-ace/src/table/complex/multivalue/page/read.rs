use super::super::{MultivalueColumn, read_multivalue_column};
use super::{MultivalueTablePage, metadata::PageMetadata};
use crate::{
    InspectError, TablePageRequest,
    header::check,
    inspect_table_features, inspect_table_schema,
    table::{calculated, page, snapshot},
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{Column, NameComparison, ScalarType, validate_columns};

/// Browse all columns with explicit multivalue support and optional cached results.
/// Every column must qualify. Parent keys and each complete child snapshot are
/// validated under existing collection/reference/capacity limits, even for empty
/// requests. Ordinary scalar payloads outside the page remain unchecked. No
/// lookup, formula, attachment/history content or external provider executes.
/// Use a stable/offline source; no shared-file coherence or partial success occurs.
pub fn read_multivalue_table_page<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    request: TablePageRequest,
    include_stored_calculations: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<MultivalueTablePage, InspectError> {
    let metadata = if include_stored_calculations {
        PageMetadata::Features(inspect_table_features(
            source, scope, table, budget, cancelled,
        )?)
    } else {
        PageMetadata::Schema(inspect_table_schema(
            source, scope, table, budget, cancelled,
        )?)
    };
    let schema = metadata.schema();
    let count = schema.columns().len();
    budget.charge_allocation(count as u64, size_of::<Column>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    let mut collections = Vec::new();
    for (position, native) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let carrier = if native.complex_type_id().is_some() {
            let collection =
                read_multivalue_column(source, scope, table, native.name(), budget, cancelled)?;
            let owner = collection.links().schema().owner();
            budget.charge(Resource::Steps, schema.columns().len() as u64)?;
            if owner.header != schema.header
                || owner.object_id() != schema.object_id()
                || owner.definition_page() != schema.definition_page()
                || owner.name() != schema.name()
                || owner.layout != schema.layout
            {
                return Err(InspectError::Malformed);
            }
            budget.charge_allocation(1, size_of::<(usize, MultivalueColumn)>() as u64)?;
            collections
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            collections.push((position, collection));
            snapshot::projected_column(native, ScalarType::Int32, budget)?
        } else if native.scalar_type().is_none() && include_stored_calculations {
            let field = metadata
                .features()
                .and_then(|f| f.columns().nth(position))
                .ok_or(InspectError::Malformed)?;
            snapshot::projected_column(native, calculated::qualified_kind(&field)?, budget)?
        } else {
            budget.charge_allocation(native.name().len() as u64, 1)?;
            native.column()?
        };
        columns.push(carrier);
    }
    validate_columns(&columns, NameComparison::AsciiInsensitive)?;
    let records = page::read_records(source, schema, &columns, request, budget, cancelled)?;
    for (position, collection) in &collections {
        for (index, row) in records.rows.iter().enumerate() {
            check(budget, cancelled)?;
            let parent = (request.offset() as usize)
                .checked_add(index)
                .and_then(|ordinal| collection.links().parents().get(ordinal))
                .ok_or(InspectError::Malformed)?;
            match (row.get(*position), parent.key()) {
                (Some(Value::Null), None) => {}
                (Some(Value::Int32(actual)), Some(key)) if *actual == key => {}
                _ => return Err(InspectError::Malformed),
            }
        }
    }
    check(budget, cancelled)?;
    Ok(MultivalueTablePage {
        metadata,
        records,
        request,
        collections,
    })
}

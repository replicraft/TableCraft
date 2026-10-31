//! Complete candidate authority rebuilt from original records and requests.
use super::super::{copy, name_work, source, unique};
use crate::{
    EditError, InspectError, NativeColumn, NativeImage, SlotScalarUpdate, TableSnapshot,
    WriteBlocker,
    format::reader::Reader,
    header::check,
    inspect_table_properties,
    preservation::COPY_CHUNK_BYTES,
    read_table_projection,
    table::{
        mutation::{slot::storage, variable_update::record_proof},
        record,
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Observed<'a> {
    pub bytes: &'a [u8],
    pub changed_rows: usize,
}

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    column: &str,
    updates: &[SlotScalarUpdate],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
        return Err(InspectError::StructureLimit.into());
    }
    let original = source::capture(image, table, budget, cancelled)?;
    let schema = original.table.schema();
    name_work(schema.name(), table, budget)?;
    if !schema.name().eq_ignore_ascii_case(table) {
        return Err(EditError::Preservation);
    }

    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), column, budget)?;
        if field.name().eq_ignore_ascii_case(column) && selected.replace(position).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[selected];
    if !field.is_ordinary_short_text() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    storage::qualify(field)?;

    for dependency in &original.dependencies {
        check(budget, cancelled)?;
        if *dependency == field.number() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    for index in original.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == field.number() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }

    let mut expected = copy(image.as_bytes(), budget, cancelled)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };

    for (position, request) in updates.iter().enumerate() {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if updates[..position]
            .iter()
            .any(|prior| prior.row() == request.row())
        {
            return Err(EditError::Conflict);
        }
        validate(field, request.expected(), reader.budget, cancelled)?;
        validate(field, request.value(), reader.budget, cancelled)?;
        let row = original
            .records
            .get(request.row())
            .ok_or(EditError::Conflict)?;
        let cells = record::decode_selected(&row.data, schema, Some(&[selected]), &mut reader)?;
        let [actual] = cells.as_slice() else {
            return Err(EditError::Preservation);
        };
        validate(field, actual, reader.budget, cancelled)?;
        if !equal(actual, request.expected(), reader.budget, cancelled)? {
            return Err(EditError::Conflict);
        }
        if equal(actual, request.value(), reader.budget, cancelled)? {
            // Rebuilding equal text could recompress it. Rebuilding absent
            // text could erase inactive bytes. A no-op preserves both exactly.
            continue;
        }

        // This independent original-record serializer is separate from the
        // producer's supplied-payload composer and receives no staged caches.
        let replacement = record_proof::reconstruct_declared(
            schema,
            selected,
            &row.data,
            request.value(),
            reader.budget,
            cancelled,
        )?;
        if replacement.len() != row.data.len() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let final_cells =
            record::decode_selected(&replacement, schema, Some(&[selected]), &mut reader)?;
        let [final_value] = final_cells.as_slice() else {
            return Err(EditError::Preservation);
        };
        if !equal(final_value, request.value(), reader.budget, cancelled)? {
            return Err(EditError::Preservation);
        }
        let end = row
            .offset
            .checked_add(replacement.len())
            .ok_or(EditError::Preservation)?;
        expected
            .get_mut(row.offset..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&replacement);
        changed.push(row.reference);
    }

    if unique(&mut changed, reader.budget, cancelled)? != observed.changed_rows
        || expected.len() != observed.bytes.len()
    {
        return Err(EditError::Preservation);
    }
    validate_expected(
        image,
        table,
        &original.table,
        &expected,
        reader.budget,
        cancelled,
    )?;
    for (expected, candidate) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(observed.bytes.chunks(COPY_CHUNK_BYTES))
    {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != candidate {
            return Err(EditError::Preservation);
        }
    }
    reader.tick()?;
    Ok(())
}

fn validate_expected(
    image: &NativeImage,
    table: &str,
    original: &TableSnapshot,
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(original.columns().len() as u64, size_of::<&str>() as u64)
        .map_err(InspectError::from)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(original.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for column in original.columns() {
        check(budget, cancelled)?;
        names.push(column.name());
    }

    let scope = image.catalog().catalog().scope();
    let projected = read_table_projection(
        &mut Cursor::new(expected),
        scope,
        table,
        &names,
        budget,
        cancelled,
    )?;
    if projected.schema().object_id() != original.schema().object_id()
        || projected.schema().definition_page() != original.schema().definition_page()
        || projected.rows().len() != original.rows().len()
        || projected.columns().len() != original.columns().len()
    {
        return Err(EditError::Preservation);
    }
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    crate::constraints::check_scalar_projection(&projected, &properties, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn validate(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if !matches!(value, Value::Text(_) | Value::Null) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    budget
        .charge(Resource::Steps, width(value))
        .map_err(InspectError::from)?;
    storage::validate(field, value)
}

fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let work = width(left)
        .checked_add(width(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

fn width(value: &Value) -> u64 {
    match value {
        Value::Text(value) => value.len() as u64,
        _ => size_of::<Value>() as u64,
    }
}

#[cfg(test)]
mod tests;

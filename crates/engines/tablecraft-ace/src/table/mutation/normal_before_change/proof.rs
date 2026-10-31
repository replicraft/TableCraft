//! Independent original-intent reconstruction; staged patches are never inputs.
use super::super::{EditError, Int32Update, NativeEditPlan, WriteBlocker, rows};
use super::{prepare, protection::Protection, qualify, validate_intents};
use crate::{
    InspectError, NativeImage,
    format::{
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    read_table,
    table::indexes::mutation::normal_index_audit,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let checked = qualify(&image, table, column, budget, cancelled)?;
    let schema = checked.schema();
    let snapshot = read_table(&mut image.reader(), scope, table, budget, cancelled)?;
    validate_intents(&snapshot, column, updates, budget, cancelled)?;
    let prepared = prepare(
        &image,
        schema.object_id(),
        column,
        updates,
        budget,
        cancelled,
    )?;
    if prepared.rejection().is_some() {
        return Err(EditError::Preservation);
    }
    let final_rows = prepared.final_rows().ok_or(EditError::Preservation)?;
    for pair in final_rows.windows(2) {
        check(budget, cancelled)?;
        if pair[0].ordinal() >= pair[1].ordinal() {
            return Err(EditError::Preservation);
        }
    }
    let protection = Protection::capture(&image, schema, budget, cancelled)?;
    let before = image.as_bytes();
    budget
        .charge_allocation(before.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(before.len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        expected.extend_from_slice(chunk);
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut visited = 0_usize;
    let mut changed = 0_usize;
    rows::visit_with_budget(&mut reader, schema, updates, |budget, row, update| {
        check(budget, cancelled)?;
        let work = final_rows.len().checked_ilog2().unwrap_or(0) as u64 + 1;
        budget
            .charge(Resource::Steps, work)
            .map_err(InspectError::from)?;
        let index = final_rows
            .binary_search_by_key(&update.row(), |row| row.ordinal())
            .map_err(|_| EditError::Preservation)?;
        let final_row = &final_rows[index];
        if final_row.values().len() != schema.columns().len() {
            return Err(EditError::Preservation);
        }
        let mut row_changed = false;
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let Some(Value::Int32(after)) = final_row.values().get(position) else {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            };
            let start = COLUMN_COUNT_BYTES + field.field.offset;
            let end = start
                .checked_add(size_of::<i32>())
                .ok_or(EditError::Preservation)?;
            let mask = row.mask + field.number() / MASK_BITS;
            if end > row.mask
                || row
                    .data
                    .get(mask)
                    .is_none_or(|value| value & (1 << (field.number() % MASK_BITS)) == 0)
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            let original = i32::from_le_bytes(
                row.data[start..end]
                    .try_into()
                    .map_err(|_| EditError::Preservation)?,
            );
            if snapshot
                .rows()
                .get(update.row())
                .and_then(|row| row.get(position))
                != Some(&Value::Int32(original))
            {
                return Err(EditError::Preservation);
            }
            if original != *after {
                let offset = row
                    .offset
                    .checked_add(start)
                    .ok_or(EditError::Preservation)?;
                protection.require(&image, schema, offset, size_of::<i32>(), budget, cancelled)?;
                expected
                    .get_mut(offset..offset + size_of::<i32>())
                    .ok_or(EditError::Preservation)?
                    .copy_from_slice(&after.to_le_bytes());
                row_changed = true;
            }
        }
        visited = visited.checked_add(1).ok_or(InspectError::StructureLimit)?;
        if row_changed {
            changed = changed.checked_add(1).ok_or(InspectError::StructureLimit)?;
        }
        Ok(())
    })?;
    if !checked.indexes().is_empty() {
        normal_index_audit::reconstruct(
            &image,
            table,
            &prepared,
            &mut expected,
            reader.budget,
            cancelled,
        )?;
    }
    if visited != final_rows.len()
        || changed != plan.changed_rows()
        || expected.len() != plan.after_bytes().len()
    {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(plan.after_bytes().chunks(COPY_CHUNK_BYTES))
    {
        check(reader.budget, cancelled)?;
        reader
            .budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    check(reader.budget, cancelled)?;
    Ok(())
}

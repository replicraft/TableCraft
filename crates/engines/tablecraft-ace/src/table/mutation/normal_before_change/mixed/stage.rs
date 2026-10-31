//! One original-image fixed payload pass; no chained candidates or INDEX edits.
use super::super::super::{
    EditError, WriteBlocker, field, guards::buffer, preserved_source::Source,
};
use crate::table::mutation::guards::fields::batch::{CheckedTargets, Prepared};
use crate::{
    InspectError, NativeImage,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(super) struct Staged {
    pub bytes: Vec<u8>,
    pub changed_rows: usize,
}

pub(super) fn target(
    source: &Source,
    position: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    target_with_indexes(source, position, None, budget, cancelled)
}

pub(super) fn target_with_indexes(
    source: &Source,
    position: usize,
    indexed: Option<&CheckedTargets<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = source.table.schema();
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    field::fixed_column_at(schema, position, ScalarType::Int32, size_of::<i32>())?;
    let field = schema
        .columns()
        .get(position)
        .ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, source.dependencies.len() as u64)
        .map_err(InspectError::from)?;
    if source.dependencies.contains(&position) {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    for index in source.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            if key.number() == field.number() {
                if let Some(indexed) = indexed {
                    indexed.require_target(schema, position, budget, cancelled)?;
                } else {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn derive(
    image: &NativeImage,
    source: &Source,
    prepared: &Prepared<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    derive_with_indexes(image, source, prepared, None, budget, cancelled)
}

pub(super) fn derive_with_indexes(
    image: &NativeImage,
    source: &Source,
    prepared: &Prepared<'_>,
    indexed: Option<&CheckedTargets<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    prepared.require_source(image, source.table.schema(), budget, cancelled)?;
    let groups = prepared.final_groups().ok_or(EditError::Preservation)?;
    let mut bytes = buffer::vector(image.as_bytes().len(), budget, cancelled)?;
    for chunk in image.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        bytes.extend_from_slice(chunk);
    }
    for group in groups {
        target_with_indexes(source, group.column(), indexed, budget, cancelled)?;
    }
    let mut changed_rows = 0_usize;
    for (ordinal, record) in source.records.iter().enumerate() {
        check(budget, cancelled)?;
        let layout = record_layout::parse(&record.data, source.table.schema(), budget)?;
        let mut changed = false;
        for group in groups {
            let field = source
                .table
                .schema()
                .columns()
                .get(group.column())
                .ok_or(EditError::Preservation)?;
            for update in group.updates() {
                check(budget, cancelled)?;
                if update.row() >= source.records.len() {
                    return Err(EditError::Conflict);
                }
                if update.row() != ordinal {
                    continue;
                }
                let (Value::Int32(before), Value::Int32(after)) =
                    (update.expected(), update.value())
                else {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                };
                budget
                    .charge(Resource::Steps, (2 * size_of::<i32>()) as u64)
                    .map_err(InspectError::from)?;
                let start = COLUMN_COUNT_BYTES
                    .checked_add(field.field.offset)
                    .ok_or(EditError::Preservation)?;
                let end = start
                    .checked_add(size_of::<i32>())
                    .ok_or(EditError::Preservation)?;
                let present = record
                    .data
                    .get(layout.mask_start + field.number() / MASK_BITS)
                    .is_some_and(|byte| byte & (1 << (field.number() % MASK_BITS)) != 0);
                if end > layout.mask_start
                    || !present
                    || record.data.get(start..end) != Some(before.to_le_bytes().as_slice())
                {
                    return Err(EditError::Conflict);
                }
                let offset = record
                    .offset
                    .checked_add(start)
                    .ok_or(EditError::Preservation)?;
                let end = offset
                    .checked_add(size_of::<i32>())
                    .ok_or(EditError::Preservation)?;
                let selected = bytes.get_mut(offset..end).ok_or(EditError::Preservation)?;
                if selected != before.to_le_bytes() {
                    return Err(EditError::Preservation);
                }
                selected.copy_from_slice(&after.to_le_bytes());
                changed |= before != after;
            }
        }
        changed_rows += usize::from(changed);
    }
    check(budget, cancelled)?;
    Ok(Staged {
        bytes,
        changed_rows,
    })
}

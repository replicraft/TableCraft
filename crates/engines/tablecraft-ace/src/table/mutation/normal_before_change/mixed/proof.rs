//! Fresh original-intent replay and an independent four-byte-only full image.
use super::super::super::{
    EditError, Int32Update, NativeEditPlan, WriteBlocker,
    guards::buffer,
    preserved_source::{Source, validation},
};
use super::super::operands::original_frames;
use super::{capture, prepare, validate};
use crate::table::mutation::guards::fields::batch::{CheckedTargets, Prepared};
use crate::{
    FixedScalarUpdate, InspectError, NativeColumn, NativeImage,
    format::{
        data_type::INT32,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::record_layout,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn target_with_indexes<'source>(
    source: &'source Source,
    position: usize,
    indexed: Option<&CheckedTargets<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'source NativeColumn, EditError> {
    let schema = source.table.schema();
    let field = schema
        .columns()
        .get(position)
        .ok_or(EditError::Preservation)?;
    if field.native_type() != INT32
        || field.scalar_type() != Some(ScalarType::Int32)
        || !field.fixed()
        || field.storage_length() != size_of::<i32>()
        || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || field.extended_flags() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

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

    let start = field.field.offset;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    for other in schema.columns() {
        check(budget, cancelled)?;
        if other.number() == field.number() || !other.fixed() || other.storage_length() == 0 {
            continue;
        }
        let other_end = other
            .field
            .offset
            .checked_add(other.storage_length())
            .ok_or(EditError::Preservation)?;
        if start < other_end && other.field.offset < end {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    Ok(field)
}

fn overlay(
    source: &Source,
    field: &NativeColumn,
    update: &FixedScalarUpdate,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let record = source
        .records
        .get(update.row())
        .ok_or(EditError::Conflict)?;
    let layout = record_layout::parse(&record.data, source.table.schema(), budget)?;
    let (Value::Int32(old), Value::Int32(new)) = (update.expected(), update.value()) else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };

    let start = COLUMN_COUNT_BYTES
        .checked_add(field.field.offset)
        .ok_or(EditError::Preservation)?;
    let end = start
        .checked_add(size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    let mask = layout
        .mask_start
        .checked_add(field.number() / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    let present = record
        .data
        .get(mask)
        .is_some_and(|byte| byte & (1 << (field.number() % MASK_BITS)) != 0);
    if end > layout.mask_start || !present {
        return Err(EditError::Conflict);
    }

    budget
        .charge(Resource::Steps, size_of::<i32>() as u64)
        .map_err(InspectError::from)?;
    let raw = record.data.get(start..end).ok_or(EditError::Preservation)?;
    let original = i32::from_le_bytes(raw.try_into().map_err(|_| EditError::Preservation)?);
    if original != *old {
        return Err(EditError::Conflict);
    }

    let at = record
        .offset
        .checked_add(start)
        .ok_or(EditError::Preservation)?;
    let stop = at
        .checked_add(size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    let bytes = expected.get_mut(at..stop).ok_or(EditError::Preservation)?;
    // Independent of the stage's to_le_bytes codec and encoded patch storage.
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = ((*new as u32) >> (index * u8::BITS as usize)) as u8;
    }
    Ok(())
}

fn changed_rows(
    source: &Source,
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut changed = 0_usize;
    for record in &source.records {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, record.data.len() as u64)
            .map_err(InspectError::from)?;
        let end = record
            .offset
            .checked_add(record.data.len())
            .ok_or(EditError::Preservation)?;
        if expected.get(record.offset..end) != Some(record.data.as_slice()) {
            changed = changed.checked_add(1).ok_or(EditError::Preservation)?;
        }
    }
    Ok(changed)
}

pub(super) fn prove(
    plan: &NativeEditPlan,
    scope: CatalogScope,
    table: &str,
    column: &str,
    updates: &[Int32Update],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.before_change.normal.mixed.prove").entered();
    let image = NativeImage::capture(
        &mut Cursor::new(plan.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let source = capture(&image, table, budget, cancelled)?;
    original_frames(&source, budget, cancelled)?;

    prepare(&image, &source, column, &[], budget, cancelled)?;
    validate(&source, column, updates, budget, cancelled)?;
    let prepared = prepare(&image, &source, column, updates, budget, cancelled)?;
    if prepared.rejection().is_some() {
        return Err(EditError::Preservation);
    }
    let (expected, changed) =
        reconstruct_source(&image, &source, &prepared, None, budget, cancelled)?;
    validation::expected(&image, table, &source.table, &expected, budget, cancelled)?;
    if expected.len() != plan.after_bytes().len() || changed != plan.changed_rows() {
        return Err(EditError::Preservation);
    }
    for (expected, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(plan.after_bytes().chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, expected.len() as u64)
            .map_err(InspectError::from)?;
        if expected != actual {
            return Err(EditError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn reconstruct_source(
    image: &NativeImage,
    source: &Source,
    prepared: &Prepared<'_>,
    indexed: Option<&CheckedTargets<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, usize), EditError> {
    prepared.require_source(image, source.table.schema(), budget, cancelled)?;
    let groups = prepared.final_groups().ok_or(EditError::Preservation)?;

    let mut expected = buffer::vector(image.as_bytes().len(), budget, cancelled)?;
    for chunk in image.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        expected.extend_from_slice(chunk);
    }

    // These are fresh execution results from original requests, never stage
    // groups, patches, frames, masks or a caller-provided selected-field list.
    for group in groups {
        let field = target_with_indexes(source, group.column(), indexed, budget, cancelled)?;
        for (position, update) in group.updates().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if group.updates()[..position]
                .iter()
                .any(|prior| prior.row() == update.row())
            {
                return Err(EditError::Conflict);
            }
            overlay(source, field, update, &mut expected, budget, cancelled)?;
        }
    }

    let changed = changed_rows(source, &expected, budget, cancelled)?;
    Ok((expected, changed))
}

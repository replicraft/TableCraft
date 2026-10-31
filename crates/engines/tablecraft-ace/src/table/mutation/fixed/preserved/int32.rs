//! Nonindexed, calculation-independent fixed Int32 preservation.
mod nullable;
pub(super) mod proof;

use super::{FixedColumnUpdates, name_work, source::Source, unique};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::mutation::{
        field,
        patch::{BytePatch, spans},
        rows,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const PAYLOAD_BYTES: usize = size_of::<i32>();

pub(super) fn requested(groups: &[FixedColumnUpdates<'_>]) -> bool {
    let [group] = groups else {
        return false;
    };
    group
        .updates()
        .first()
        .is_some_and(|update| matches!(update.expected(), Value::Int32(_) | Value::Null))
}

/// Only metadata qualification may select the empty numeric path. This helper
/// cannot swallow cancellation, budget, row or conflict errors from staging.
pub(super) fn empty_target(
    schema: &TableSchema,
    group: &FixedColumnUpdates<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    for field in schema.columns() {
        name_work(field.name(), group.column(), budget)?;
    }
    match field::fixed_column(schema, group.column(), ScalarType::Int32, PAYLOAD_BYTES) {
        Ok(_) => Ok(true),
        Err(EditError::Unsupported(WriteBlocker::Column)) => Ok(false),
        Err(error) => Err(error),
    }
}

struct Payload {
    offset: usize,
    before: [u8; PAYLOAD_BYTES],
    after: [u8; PAYLOAD_BYTES],
}

impl BytePatch for Payload {
    fn offset(&self) -> usize {
        self.offset
    }

    fn before(&self) -> &[u8] {
        &self.before
    }

    fn after(&self) -> &[u8] {
        &self.after
    }
}

pub(super) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    let [group] = groups else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    let schema = source.table.schema();
    for field in schema.columns() {
        name_work(field.name(), group.column(), budget)?;
    }
    let column = field::fixed_column(schema, group.column(), ScalarType::Int32, PAYLOAD_BYTES)?;
    prepare_bound(image, source, group, column, budget, cancelled)
}

pub(super) fn prepare_bound(
    image: &NativeImage,
    source: &Source,
    group: &FixedColumnUpdates<'_>,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    let schema = source.table.schema();
    budget
        .charge(
            Resource::Steps,
            (source.dependencies.len()
                + source
                    .indexes
                    .indexes()
                    .iter()
                    .map(|index| index.columns().len())
                    .sum::<usize>()) as u64,
        )
        .map_err(InspectError::from)?;
    if source.dependencies.contains(&column)
        || source
            .indexes
            .indexes()
            .iter()
            .any(|index| index.columns().iter().any(|key| key.number() == column))
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    rows::unique(group.updates(), budget, cancelled)?;
    let mut has_null = false;
    for update in group.updates() {
        check(budget, cancelled)?;
        match (update.expected(), update.value()) {
            (Value::Int32(_), Value::Int32(_)) => {}
            (Value::Null, Value::Null | Value::Int32(_)) | (Value::Int32(_), Value::Null) => {
                has_null = true;
            }
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        }
    }
    if has_null {
        return nullable::prepare(image, schema, column, group, budget, cancelled);
    }

    budget
        .charge_allocation(group.updates().len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(group.updates().len())
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(group.updates().len() as u64, size_of::<Payload>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(group.updates().len())
        .map_err(|_| InspectError::Allocation)?;
    let mut patches = Vec::new();
    let at = COLUMN_COUNT_BYTES
        .checked_add(schema.columns()[column].field.offset)
        .ok_or(EditError::Preservation)?;
    let end = at
        .checked_add(PAYLOAD_BYTES)
        .ok_or(EditError::Preservation)?;

    rows::final_record::visit(
        image,
        schema,
        group.updates(),
        budget,
        cancelled,
        |row, update| {
            let (Value::Int32(expected), Value::Int32(value)) = (update.expected(), update.value())
            else {
                return Err(EditError::Preservation);
            };
            let mask = *row
                .data
                .get(row.mask + column / MASK_BITS)
                .ok_or(InspectError::Malformed)?;
            if mask & (1 << (column % MASK_BITS)) == 0 {
                return Err(EditError::Conflict);
            }
            if end > row.mask {
                return Err(EditError::Preservation);
            }
            let before: [u8; PAYLOAD_BYTES] = row
                .data
                .get(at..end)
                .ok_or(InspectError::Malformed)?
                .try_into()
                .map_err(|_| InspectError::Malformed)?;
            if i32::from_le_bytes(before) != *expected {
                return Err(EditError::Conflict);
            }
            if expected != value {
                // At most one payload per request; capacity was paid and
                // fallibly reserved before the visitor borrowed the budget.
                payloads.push(Payload {
                    offset: row.offset + at,
                    before,
                    after: value.to_le_bytes(),
                });
                changed.push(row.reference);
            }
            Ok(())
        },
    )?;
    for payload in payloads {
        spans::push(&mut patches, &payload, budget, cancelled)?;
    }
    spans::coalesce(&mut patches, budget, cancelled)?;
    let changed = unique(&mut changed, budget, cancelled)?;
    Ok((patches, changed))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod genuine_sources;

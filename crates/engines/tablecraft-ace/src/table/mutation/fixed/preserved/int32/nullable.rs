//! Original presence and payload observations for one qualified Int32 field.
use super::{FixedColumnUpdates, PAYLOAD_BYTES, Payload, unique};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    table::mutation::{
        patch::{BytePatch, spans},
        rows,
    },
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

struct Presence {
    offset: usize,
    before: [u8; 1],
    after: [u8; 1],
}

impl BytePatch for Presence {
    fn offset(&self) -> usize {
        self.offset
    }

    fn before(&self) -> &[u8] {
        &self.before
    }

    fn after(&self) -> &[u8] {
        &self.after
    }

    fn is_mask(&self) -> bool {
        true
    }
}

pub(super) fn prepare(
    image: &NativeImage,
    schema: &TableSchema,
    column: usize,
    group: &FixedColumnUpdates<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    // The caller already qualified the exact target, every carrier, source
    // index/dependency exclusions and unique requested row ordinals.
    let capacity = group.updates().len();
    budget
        .charge_allocation(capacity as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Payload>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Presence>() as u64)
        .map_err(InspectError::from)?;
    let mut presence = Vec::new();
    presence
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;

    let at = COLUMN_COUNT_BYTES
        .checked_add(schema.columns()[column].field.offset)
        .ok_or(EditError::Preservation)?;
    let end = at
        .checked_add(PAYLOAD_BYTES)
        .ok_or(EditError::Preservation)?;
    let bit = 1_u8 << (column % MASK_BITS);
    rows::final_record::visit(
        image,
        schema,
        group.updates(),
        budget,
        cancelled,
        |row, update| {
            let mask_at = row
                .mask
                .checked_add(column / MASK_BITS)
                .ok_or(EditError::Preservation)?;
            let before_mask = *row.data.get(mask_at).ok_or(InspectError::Malformed)?;
            let present = before_mask & bit != 0;
            if end > row.mask {
                return Err(EditError::Preservation);
            }
            let before: [u8; PAYLOAD_BYTES] = row
                .data
                .get(at..end)
                .ok_or(InspectError::Malformed)?
                .try_into()
                .map_err(|_| InspectError::Malformed)?;
            match update.expected() {
                Value::Null if !present => {}
                Value::Int32(expected) if present && i32::from_le_bytes(before) == *expected => {}
                Value::Null | Value::Int32(_) => return Err(EditError::Conflict),
                _ => return Err(EditError::Preservation),
            }
            let (after_mask, after_payload) = match update.value() {
                Value::Null => (before_mask & !bit, None),
                Value::Int32(value) => (before_mask | bit, Some(value.to_le_bytes())),
                _ => return Err(EditError::Preservation),
            };
            let mask_changed = before_mask != after_mask;
            let payload_changed = after_payload.is_some_and(|after| before != after);
            if mask_changed {
                presence.push(Presence {
                    offset: row.offset + mask_at,
                    before: [before_mask],
                    after: [after_mask],
                });
            }
            if let Some(after) = after_payload.filter(|after| *after != before) {
                payloads.push(Payload {
                    offset: row.offset + at,
                    before,
                    after,
                });
            }
            if mask_changed || payload_changed {
                changed.push(row.reference);
            }
            Ok(())
        },
    )?;

    // No callback captures the borrowed visitor budget; every observation fits
    // the paid per-request capacity. NULL leaves inactive payload bytes exact.
    let mut patches = Vec::new();
    for payload in payloads {
        spans::push(&mut patches, &payload, budget, cancelled)?;
    }
    for mask in presence {
        spans::push(&mut patches, &mask, budget, cancelled)?;
    }
    spans::coalesce(&mut patches, budget, cancelled)?;
    let count = unique(&mut changed, budget, cancelled)?;
    Ok((patches, count))
}

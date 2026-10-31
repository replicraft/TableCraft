//! One paid original-row visit produces joint fixed payload/mask observations.
use super::super::{FixedColumnUpdates, source::Source, unique};
use super::binding;
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeImage,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::mutation::{
        fixed::encoding,
        patch::{BytePatch, spans},
        rows::{self, RowAssignment},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const MAX_PAYLOAD_BYTES: usize = size_of::<i64>();
// Two encodings and three possible fixed-width comparisons per numeric request.
const PAYLOAD_PASSES: usize = 5;

struct Cell<'a> {
    group: usize,
    update: &'a FixedScalarUpdate,
}

struct Selected {
    row: usize,
    start: usize,
    end: usize,
}

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.row
    }
}

struct Payload {
    offset: usize,
    before: [u8; MAX_PAYLOAD_BYTES],
    after: [u8; MAX_PAYLOAD_BYTES],
    length: usize,
}

impl BytePatch for Payload {
    fn offset(&self) -> usize {
        self.offset
    }

    fn before(&self) -> &[u8] {
        &self.before[..self.length]
    }

    fn after(&self) -> &[u8] {
        &self.after[..self.length]
    }
}

struct Mask {
    offset: usize,
    before: [u8; 1],
    after: [u8; 1],
}

impl BytePatch for Mask {
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

fn encode(kind: ScalarType, value: &Value) -> Result<Option<[u8; MAX_PAYLOAD_BYTES]>, EditError> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    if kind == ScalarType::Int32 {
        let encoded = encoding::encode::<{ size_of::<i32>() }>(kind, value)?;
        let mut bytes = [0; MAX_PAYLOAD_BYTES];
        bytes[..size_of::<i32>()].copy_from_slice(&encoded);
        Ok(Some(bytes))
    } else {
        Ok(Some(encoding::encode::<MAX_PAYLOAD_BYTES>(kind, value)?))
    }
}

pub(in crate::table::mutation::fixed::preserved) fn prepare(
    image: &NativeImage,
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    let (bound, capacity) = binding::all(source, groups, budget, cancelled)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Cell<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    for (group, requests) in groups.iter().enumerate() {
        for update in requests.updates() {
            check(budget, cancelled)?;
            cells.push(Cell { group, update });
        }
    }
    let levels = capacity.checked_ilog2().unwrap_or(0) as usize + 1;
    let sorting = capacity
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, sorting as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    cells.sort_unstable_by_key(|cell| (cell.update.row(), cell.group));
    check(budget, cancelled)?;

    budget
        .charge_allocation(capacity as u64, size_of::<Selected>() as u64)
        .map_err(InspectError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut start = 0;
    while start < cells.len() {
        check(budget, cancelled)?;
        let row = cells[start].update.row();
        let mut end = start + 1;
        while end < cells.len() && cells[end].update.row() == row {
            check(budget, cancelled)?;
            end += 1;
        }
        selected.push(Selected { row, start, end });
        start = end;
    }

    budget
        .charge_allocation(selected.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(selected.len())
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Payload>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Mask>() as u64)
        .map_err(InspectError::from)?;
    let mut masks = Vec::new();
    masks
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;

    for (group, target) in groups.iter().zip(&bound) {
        check(budget, cancelled)?;
        let work = group
            .updates()
            .len()
            .checked_mul(target.width.max(1))
            .and_then(|bytes| bytes.checked_mul(PAYLOAD_PASSES))
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
    }

    let schema = source.table.schema();
    if !selected.is_empty() {
        rows::final_record::visit(
            image,
            schema,
            &selected,
            budget,
            cancelled,
            |row, selection| {
                let mut row_changed = false;
                for cell in &cells[selection.start..selection.end] {
                    if cancelled() {
                        return Err(InspectError::Cancelled.into());
                    }
                    let target = &bound[cell.group];
                    let update = cell.update;
                    let mask_at = row
                        .mask
                        .checked_add(target.column / MASK_BITS)
                        .ok_or(EditError::Preservation)?;
                    let original_mask = *row.data.get(mask_at).ok_or(InspectError::Malformed)?;
                    let bit = 1_u8 << (target.column % MASK_BITS);
                    if target.kind == ScalarType::Boolean {
                        let (Value::Boolean(expected), Value::Boolean(value)) =
                            (update.expected(), update.value())
                        else {
                            return Err(EditError::Preservation);
                        };
                        if (original_mask & bit != 0) != *expected {
                            return Err(EditError::Conflict);
                        }
                        if expected != value {
                            let after = if *value {
                                original_mask | bit
                            } else {
                                original_mask & !bit
                            };
                            masks.push(Mask {
                                offset: row.offset + mask_at,
                                before: [original_mask],
                                after: [after],
                            });
                            row_changed = true;
                        }
                        continue;
                    }

                    let at = COLUMN_COUNT_BYTES
                        .checked_add(schema.columns()[target.column].field.offset)
                        .ok_or(EditError::Preservation)?;
                    let end = at
                        .checked_add(target.width)
                        .ok_or(EditError::Preservation)?;
                    if end > row.mask {
                        return Err(EditError::Preservation);
                    }
                    let actual = row.data.get(at..end).ok_or(InspectError::Malformed)?;
                    let expected = encode(target.kind, update.expected())?;
                    let replacement = encode(target.kind, update.value())?;
                    if (original_mask & bit != 0) != expected.is_some()
                        || expected.is_some_and(|bytes| &bytes[..target.width] != actual)
                    {
                        return Err(EditError::Conflict);
                    }
                    let after_mask = if replacement.is_some() {
                        original_mask | bit
                    } else {
                        original_mask & !bit
                    };
                    let mask_changed = after_mask != original_mask;
                    let payload_changed =
                        replacement.is_some_and(|bytes| &bytes[..target.width] != actual);
                    if mask_changed {
                        masks.push(Mask {
                            offset: row.offset + mask_at,
                            before: [original_mask],
                            after: [after_mask],
                        });
                    }
                    if let Some(after) =
                        replacement.filter(|bytes| &bytes[..target.width] != actual)
                    {
                        let mut before = [0; MAX_PAYLOAD_BYTES];
                        before[..target.width].copy_from_slice(actual);
                        payloads.push(Payload {
                            offset: row.offset + at,
                            before,
                            after,
                            length: target.width,
                        });
                    }
                    row_changed |= mask_changed || payload_changed;
                }
                if row_changed {
                    changed.push(row.reference);
                }
                Ok(())
            },
        )?;
    }
    let mut patches = Vec::new();
    for payload in payloads {
        spans::push(&mut patches, &payload, budget, cancelled)?;
    }
    for mask in masks {
        spans::push(&mut patches, &mask, budget, cancelled)?;
    }
    // Every mask contribution retains the same ORIGINAL before byte. The owning
    // coalescer merges only disjoint bits and rejects all payload overlap.
    spans::coalesce(&mut patches, budget, cancelled)?;
    let count = unique(&mut changed, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok((patches, count))
}

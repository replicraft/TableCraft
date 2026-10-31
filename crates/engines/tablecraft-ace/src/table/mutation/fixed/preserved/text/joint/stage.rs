//! Compose one complete original record; fixed bits follow Text serialization.
use super::super::{
    super::{copy, joint::binding::Bound as FixedColumn, source::Source, unique},
    stage as text,
};
use super::binding::{self, Kind};
use crate::{
    EditError, FittedColumnUpdates, InspectError,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::{
        mutation::{fixed::encoding, patch::BytePatch},
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const MAX_PAYLOAD_BYTES: usize = size_of::<i64>();
// Two observations each encode and compare both carriers; the overlay copies once.
const PAYLOAD_PASSES: usize = 10;

struct Cell<'a> {
    row: usize,
    group: usize,
    expected: &'a Value,
    value: &'a Value,
}

pub(in crate::table::mutation::fixed::preserved::text) struct Rebuilt<'a> {
    offset: usize,
    before: &'a [u8],
    after: Vec<u8>,
}

impl BytePatch for Rebuilt<'_> {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        self.before
    }
    fn after(&self) -> &[u8] {
        &self.after
    }
}

struct FixedValue {
    at: usize,
    width: usize,
    mask_at: usize,
    bit: u8,
    present: bool,
    payload: Option<[u8; MAX_PAYLOAD_BYTES]>,
    changed: bool,
}

fn encode(kind: ScalarType, value: &Value) -> Result<Option<[u8; MAX_PAYLOAD_BYTES]>, EditError> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    if kind == ScalarType::Int32 {
        let small = encoding::encode::<{ size_of::<i32>() }>(kind, value)?;
        let mut bytes = [0; MAX_PAYLOAD_BYTES];
        bytes[..small.len()].copy_from_slice(&small);
        Ok(Some(bytes))
    } else {
        Ok(Some(encoding::encode::<MAX_PAYLOAD_BYTES>(kind, value)?))
    }
}

// All stale checks use the unchanged original, never the accumulating output.
fn observe(
    before: &[u8],
    mask: usize,
    target: &FixedColumn,
    field_offset: usize,
    cell: &Cell<'_>,
) -> Result<FixedValue, EditError> {
    let mask_at = mask
        .checked_add(target.column / MASK_BITS)
        .ok_or(EditError::Preservation)?;
    let bit = 1_u8 << (target.column % MASK_BITS);
    let original = *before.get(mask_at).ok_or(InspectError::Malformed)?;
    if target.kind == ScalarType::Boolean {
        let (Value::Boolean(expected), Value::Boolean(value)) = (cell.expected, cell.value) else {
            return Err(EditError::Preservation);
        };
        if (original & bit != 0) != *expected {
            return Err(EditError::Conflict);
        }
        return Ok(FixedValue {
            at: 0,
            width: 0,
            mask_at,
            bit,
            present: *value,
            payload: None,
            changed: expected != value,
        });
    }
    let at = COLUMN_COUNT_BYTES
        .checked_add(field_offset)
        .ok_or(EditError::Preservation)?;
    let end = at
        .checked_add(target.width)
        .ok_or(EditError::Preservation)?;
    if end > mask {
        return Err(EditError::Preservation);
    }
    let actual = before.get(at..end).ok_or(InspectError::Malformed)?;
    let expected = encode(target.kind, cell.expected)?;
    let payload = encode(target.kind, cell.value)?;
    if (original & bit != 0) != expected.is_some()
        || expected.is_some_and(|bytes| &bytes[..target.width] != actual)
    {
        return Err(EditError::Conflict);
    }
    let present = payload.is_some();
    let changed = (original & bit != 0) != present
        || payload.is_some_and(|bytes| &bytes[..target.width] != actual);
    Ok(FixedValue {
        at,
        width: target.width,
        mask_at,
        bit,
        present,
        payload,
        changed,
    })
}

fn overlay(after: &mut [u8], value: FixedValue) -> Result<(), EditError> {
    let mask = after
        .get_mut(value.mask_at)
        .ok_or(EditError::Preservation)?;
    if value.present {
        *mask |= value.bit;
    } else {
        *mask &= !value.bit;
    }
    if let Some(payload) = value.payload {
        let end = value
            .at
            .checked_add(value.width)
            .ok_or(EditError::Preservation)?;
        after
            .get_mut(value.at..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&payload[..value.width]);
    }
    Ok(())
}

pub(in crate::table::mutation::fixed::preserved::text) fn prepare<'a>(
    original: &'a Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Rebuilt<'a>>, usize), EditError> {
    let (bound, capacity) = binding::all(original, groups, budget, cancelled)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Cell<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    for (group, requests) in groups.iter().enumerate() {
        match requests {
            FittedColumnUpdates::Fixed { updates, .. } => {
                for update in *updates {
                    check(budget, cancelled)?;
                    cells.push(Cell {
                        row: update.row(),
                        group,
                        expected: update.expected(),
                        value: update.value(),
                    });
                }
            }
            FittedColumnUpdates::Variable { updates, .. } => {
                for update in *updates {
                    check(budget, cancelled)?;
                    cells.push(Cell {
                        row: update.row(),
                        group,
                        expected: update.expected(),
                        value: update.value(),
                    });
                }
            }
        }
    }
    let work = capacity
        .checked_mul(capacity.checked_ilog2().unwrap_or(0) as usize + 1)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    cells.sort_unstable_by_key(|cell| (cell.row, cell.group));
    check(budget, cancelled)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Rebuilt<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches: Vec<Rebuilt<'_>> = Vec::new();
    patches
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(capacity as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let schema = original.table.schema();
    let mut start = 0;
    while start < cells.len() {
        check(budget, cancelled)?;
        let ordinal = cells[start].row;
        let mut end = start + 1;
        while end < cells.len() && cells[end].row == ordinal {
            check(budget, cancelled)?;
            end += 1;
        }
        let source = original.records.get(ordinal).ok_or(EditError::Conflict)?;
        let layout = record_layout::parse(&source.data, schema, budget)?;
        let mut text_change = None;
        let mut row_changed = false;
        for cell in &cells[start..end] {
            check(budget, cancelled)?;
            let target = &bound[cell.group];
            match &target.kind {
                Kind::Text { projected } => {
                    let actual = original
                        .table
                        .rows()
                        .get(ordinal)
                        .and_then(|row| row.get(*projected))
                        .ok_or(EditError::Conflict)?;
                    text::validate(&schema.columns()[target.column], actual, budget, cancelled)?;
                    if !text::equal(actual, cell.expected, budget, cancelled)? {
                        return Err(EditError::Conflict);
                    }
                    if !text::equal(cell.expected, cell.value, budget, cancelled)? {
                        text_change = Some((target.column, cell.value));
                        row_changed = true;
                    }
                }
                Kind::Fixed(fixed) => {
                    let work = fixed
                        .width
                        .max(1)
                        .checked_mul(PAYLOAD_PASSES)
                        .ok_or(InspectError::StructureLimit)?;
                    budget
                        .charge(Resource::Steps, work as u64)
                        .map_err(InspectError::from)?;
                    row_changed |= observe(
                        &source.data,
                        layout.mask_start,
                        fixed,
                        schema.columns()[target.column].field.offset,
                        cell,
                    )?
                    .changed;
                }
            }
        }
        if row_changed {
            let mut after = if let Some((column, value)) = text_change {
                text::rebuild(schema, column, &source.data, value, budget, cancelled)?
            } else {
                copy(&source.data, budget, cancelled)?
            };
            for cell in &cells[start..end] {
                check(budget, cancelled)?;
                let target = &bound[cell.group];
                if let Kind::Fixed(fixed) = &target.kind {
                    // The earlier paid width bound includes this repeated stack encoding.
                    let value = observe(
                        &source.data,
                        layout.mask_start,
                        fixed,
                        schema.columns()[target.column].field.offset,
                        cell,
                    )?;
                    overlay(&mut after, value)?;
                }
            }
            let source_end = source
                .offset
                .checked_add(source.data.len())
                .ok_or(EditError::Preservation)?;
            for prior in &patches {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, 1)
                    .map_err(InspectError::from)?;
                let prior_end = prior
                    .offset
                    .checked_add(prior.before.len())
                    .ok_or(EditError::Preservation)?;
                if source.offset < prior_end && prior.offset < source_end {
                    return Err(EditError::Preservation);
                }
            }
            patches.push(Rebuilt {
                offset: source.offset,
                before: &source.data,
                after,
            });
            changed.push(source.reference);
        }
        start = end;
    }
    let count = unique(&mut changed, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok((patches, count))
}

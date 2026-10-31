//! Exact original presence and eight-byte payload observations, then paid patches.
use super::super::{FixedColumnUpdates, source::Source, unique};
use super::binding::{self, PAYLOAD_BYTES};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::row::{COLUMN_COUNT_BYTES, MASK_BITS},
    header::check,
    table::mutation::{
        field,
        fixed::encoding,
        patch::{BytePatch, spans},
        rows,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

// Two encodings and three possible fixed-width comparisons in the visitor.
const PAYLOAD_WORK_PER_ROW: usize = PAYLOAD_BYTES * 5;

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

pub(in crate::table::mutation::fixed::preserved) fn prepare(
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
    let column = binding::position(schema, group, budget)?;
    let kind = binding::native_kind(&schema.columns()[column])?;
    prepare_bound(image, source, group, column, kind, budget, cancelled)
}

pub(in crate::table::mutation::fixed::preserved) fn prepare_bound(
    image: &NativeImage,
    source: &Source,
    group: &FixedColumnUpdates<'_>,
    column: usize,
    kind: ScalarType,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, usize), EditError> {
    let schema = source.table.schema();
    let selected = schema
        .columns()
        .get(column)
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if binding::native_kind(selected)? != kind {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    field::fixed_column_at(schema, column, kind, PAYLOAD_BYTES)?;
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
    for update in group.updates() {
        check(budget, cancelled)?;
        for value in [update.expected(), update.value()] {
            if !matches!(
                (kind, value),
                (_, Value::Null)
                    | (ScalarType::Int64, Value::Int64(_))
                    | (ScalarType::Currency, Value::Currency(_))
                    | (ScalarType::DateTime, Value::DateTime(_))
            ) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
    }
    let capacity = group.updates().len();
    let work = capacity
        .checked_mul(PAYLOAD_WORK_PER_ROW)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
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
        .checked_add(selected.field.offset)
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
                Value::Null => return Err(EditError::Conflict),
                expected
                    if present && encoding::encode::<PAYLOAD_BYTES>(kind, expected)? == before => {}
                _ => return Err(EditError::Conflict),
            }
            let after_payload = if matches!(update.value(), Value::Null) {
                None
            } else {
                Some(encoding::encode::<PAYLOAD_BYTES>(kind, update.value())?)
            };
            let after_mask = if after_payload.is_some() {
                before_mask | bit
            } else {
                before_mask & !bit
            };
            let mask_changed = before_mask != after_mask;
            let payload_changed = after_payload.is_some_and(|after| after != before);
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

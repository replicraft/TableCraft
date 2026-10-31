use super::super::{name_work, source::Source, unique};
use crate::{
    EditError, InspectError, NativeColumn, SlotScalarUpdate, TableSchema, WriteBlocker,
    header::check,
    table::{
        mutation::{page_update::record, patch::BytePatch, rows, slot::storage},
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Rebuilt<'a> {
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

pub(super) fn prepare<'a>(
    original: &'a Source,
    column: &str,
    updates: &[SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Rebuilt<'a>>, usize), EditError> {
    let schema = original.table.schema();
    let mut selected = None;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(field.name(), column, budget)?;
        if field.name().eq_ignore_ascii_case(column) && selected.replace(position).is_some() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    let selected = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns()[selected];
    if !field.is_ordinary_short_text() {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    storage::qualify(field)?;
    budget
        .charge(
            Resource::Steps,
            (original.dependencies.len()
                + original
                    .indexes
                    .indexes()
                    .iter()
                    .map(|index| index.columns().len())
                    .sum::<usize>()) as u64,
        )
        .map_err(InspectError::from)?;
    if original.dependencies.contains(&field.number())
        || original.indexes.indexes().iter().any(|index| {
            index
                .columns()
                .iter()
                .any(|key| key.number() == field.number())
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut projected = None;
    for (position, candidate) in original.table.columns().iter().enumerate() {
        check(budget, cancelled)?;
        name_work(candidate.name(), field.name(), budget)?;
        if candidate.name() == field.name() && projected.replace(position).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let projected = projected.ok_or(EditError::Preservation)?;
    rows::unique(updates, budget, cancelled)?;
    for update in updates {
        check(budget, cancelled)?;
        if !matches!(update.expected(), Value::Text(_) | Value::Null)
            || !matches!(update.value(), Value::Text(_) | Value::Null)
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        validate(field, update.expected(), budget, cancelled)?;
        validate(field, update.value(), budget, cancelled)?;
    }
    budget
        .charge_allocation(updates.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<Rebuilt<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches: Vec<Rebuilt<'_>> = Vec::new();
    patches
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for update in updates {
        check(budget, cancelled)?;
        let source = original
            .records
            .get(update.row())
            .ok_or(EditError::Conflict)?;
        let actual = original
            .table
            .rows()
            .get(update.row())
            .and_then(|row| row.get(projected))
            .ok_or(EditError::Conflict)?;
        validate(field, actual, budget, cancelled)?;
        if !equal(actual, update.expected(), budget, cancelled)? {
            return Err(EditError::Conflict);
        }
        if equal(update.expected(), update.value(), budget, cancelled)? {
            continue;
        }
        let after = rebuild(
            schema,
            selected,
            &source.data,
            update.value(),
            budget,
            cancelled,
        )?;
        let end = source
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
            if source.offset < prior_end && prior.offset < end {
                return Err(EditError::Preservation);
            }
        }
        // The complete record is a bounded, paid owned result. Fixed-span
        // transport is deliberately not widened for variable record patches.
        patches.push(Rebuilt {
            offset: source.offset,
            before: &source.data,
            after,
        });
        changed.push(source.reference);
    }
    let count = unique(&mut changed, budget, cancelled)?;
    Ok((patches, count))
}

pub(super) fn equal(
    left: &Value,
    right: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let width = |value: &Value| match value {
        Value::Text(text) => text.len() as u64,
        _ => size_of::<Value>() as u64,
    };
    let work = width(left)
        .checked_add(width(right))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    Ok(left == right)
}

pub(super) fn validate(
    field: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let work = match value {
        Value::Text(text) => text.len() as u64,
        _ => size_of::<Value>() as u64,
    };
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    storage::validate(field, value)
}

// Preserve the original single-field serializer and work order for both callers.
pub(super) fn rebuild(
    schema: &TableSchema,
    selected: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let layout = record_layout::parse(before, schema, budget)?;
    let start = *layout
        .offsets
        .get(schema.columns()[selected].field.variable)
        .ok_or(InspectError::Malformed)?;
    let end = *layout
        .offsets
        .get(schema.columns()[selected].field.variable + 1)
        .ok_or(InspectError::Malformed)?;
    let preserved = layout
        .offsets
        .last()
        .ok_or(InspectError::Malformed)?
        .checked_sub(end.checked_sub(start).ok_or(InspectError::Malformed)?)
        .ok_or(InspectError::Malformed)?;
    let capacity = layout
        .data_end
        .checked_sub(preserved)
        .ok_or(InspectError::Malformed)?;
    let payload = storage::encode_declared(
        &schema.columns()[selected],
        value,
        capacity,
        budget,
        cancelled,
    )?;
    let after = record::replace_payload(
        schema,
        selected,
        before,
        &payload,
        !matches!(value, Value::Null),
        budget,
        cancelled,
    )?;
    if after.len() != before.len() {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(after)
}

//! Fitting fixed integral UPDATE chosen from original native index declarations.
mod proof;

use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::data_type::{CURRENCY, INT64},
    header::check,
    table::{
        indexes::{IndexKeyUpdate, MutationKey, mutation::normal_integral_update},
        mutation::{
            fixed_records::{self, binding, indexed::integral_updates::IntegralUpdates, stage},
            patch::BytePatch,
            preserved_source::{Source, validation},
        },
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const COMPARISON_BYTES: usize = 2 * size_of::<i64>();

pub(super) fn selected(
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let schema = source.table.schema();
    let mut wide = false;
    for index in source.indexes.indexes() {
        for key in index.columns() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, schema.columns().len() as u64)
                .map_err(InspectError::from)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(EditError::Preservation)?;
            wide |= matches!(field.native_type(), INT64 | CURRENCY);
        }
    }
    Ok(wide)
}

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let facts = IntegralUpdates::capture(&image, &source, budget, cancelled)?;
    let (bound, assignments) =
        binding::all_integral(&image, &source, groups, &facts, budget, cancelled)?;
    for (group, target) in groups.iter().zip(&bound) {
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            let original = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(target.column))
                .ok_or(EditError::Conflict)?;
            if original != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    let (data, changed_rows) =
        stage::prepare_integral(&image, &source, groups, &facts, budget, cancelled)?;
    let mut updates = vector(assignments, budget, cancelled)?;
    for (group, target) in groups.iter().zip(&bound) {
        let mut indexed = false;
        for index in source.indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                indexed |= key.number() == source.table.schema().columns()[target.column].number();
            }
        }
        if !indexed {
            continue;
        }
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            if update.expected() == update.value() {
                continue;
            }
            let record = source
                .records
                .get(update.row())
                .ok_or(EditError::Conflict)?;
            updates.push(index_update(
                record.reference,
                target.column,
                target.kind,
                update.expected(),
                update.value(),
            )?);
        }
    }
    let indexes =
        normal_integral_update::patches(&image, facts.checked(), &updates, budget, cancelled)?;
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for patch in &data {
        apply(
            &mut after,
            patch.offset(),
            patch.before(),
            patch.after(),
            budget,
            cancelled,
        )?;
    }
    for patch in indexes {
        apply(
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?;
    }
    validation::expected(&image, table, &source.table, &after, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    drop(facts);
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    };
    proof::prove(&plan, scope, table, groups, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn index_update(
    reference: u32,
    column: usize,
    kind: ScalarType,
    before: &Value,
    after: &Value,
) -> Result<IndexKeyUpdate, EditError> {
    let codec = MutationKey::new(kind)?;
    let cell = |value: &Value| match (kind, value) {
        (ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency, Value::Null) => Ok(None),
        (ScalarType::Int32, Value::Int32(value)) => Ok(Some(codec.stored(&value.to_le_bytes())?)),
        (ScalarType::Int64, Value::Int64(value)) => Ok(Some(codec.stored(&value.to_le_bytes())?)),
        (ScalarType::Currency, Value::Currency(value)) => {
            Ok(Some(codec.stored(&value.scaled().to_le_bytes())?))
        }
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    Ok(IndexKeyUpdate {
        reference,
        column,
        expected: cell(before)?,
        value: cell(after)?,
    })
}

pub(super) fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

fn apply(
    after: &mut [u8],
    offset: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    let output = after.get_mut(offset..end).ok_or(EditError::Preservation)?;
    if output != before || before.len() != replacement.len() {
        return Err(EditError::Preservation);
    }
    output.copy_from_slice(replacement);
    Ok(())
}

pub(super) fn compare(
    expected: &[u8],
    actual: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if expected.len() != actual.len() {
        return Err(EditError::Preservation);
    }
    for (left, right) in expected
        .chunks(crate::preservation::COPY_CHUNK_BYTES)
        .zip(actual.chunks(crate::preservation::COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, left.len() as u64)
            .map_err(InspectError::from)?;
        if left != right {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

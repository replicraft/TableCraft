//! Preselected fitting manual-I32 INDEX maintenance from original fixed groups.
mod proof;

use crate::{
    EditError, FixedColumnUpdates, IndexCheckStatus, InspectError, NativeEditPlan, NativeImage,
    WriteBlocker, check_table,
    header::check,
    table::{
        indexes::{IndexKeyUpdate, MutationKey, mutation::normal_fitting},
        mutation::{
            fixed_records::{self, binding, indexed::CheckedIndexes, stage},
            patch::BytePatch,
            preserved_source::{Source, validation},
        },
    },
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const COMPARISON_BYTES: usize = 2 * size_of::<i64>();

/// This reads only original target names and native key metadata. A missing or
/// invalid target still reaches binding; no failed planner selects a fallback.
pub(super) fn selected(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    if source.indexes.indexes().is_empty() {
        return Ok(false);
    }
    let schema = source.table.schema();
    for group in groups {
        for field in schema.columns() {
            check(budget, cancelled)?;
            fixed_records::name_work(field.name(), group.column(), budget)?;
            if !field.name().eq_ignore_ascii_case(group.column()) {
                continue;
            }
            for index in source.indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    if key.number() == field.number() {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    groups: &[FixedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.ordinary.indexed.plan").entered();
    let facts = CheckedIndexes::capture(&image, table, budget, cancelled)?;
    let bound = preflight(&image, &source, groups, &facts, budget, cancelled)?;
    let (data, changed_rows) =
        stage::prepare_indexed(&image, &source, groups, &facts, budget, cancelled)?;
    let updates = index_updates(&source, groups, &bound, budget, cancelled)?;
    // All original-to-final keys are supplied together. UNIQUE swaps are
    // evaluated in their final population; no intermediate candidate exists.
    let indexes = normal_fitting::patches(&image, facts.checked(), &updates, budget, cancelled)?;
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    for patch in data {
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
    let checked = check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?;
    if checked.schema().object_id() != source.table.schema().object_id()
        || checked.schema().definition_page() != source.table.schema().definition_page()
        || checked.rows() != source.records.len()
        || checked.indexes().len() != facts.checked().indexes().len()
    {
        return Err(EditError::Preservation);
    }
    for index in checked.indexes() {
        check(budget, cancelled)?;
        if index.status() != IndexCheckStatus::Checked {
            return Err(EditError::Preservation);
        }
    }
    drop(facts);
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    };
    // The independent proof recaptures BEFORE/closure/Source/checked facts and
    // manually replays records plus nullable keys, not these patches or updates.
    proof::prove(&plan, scope, table, groups, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn preflight(
    image: &NativeImage,
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    facts: &CheckedIndexes<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<binding::Bound>, EditError> {
    let (bound, _) = binding::all_indexed(image, source, groups, facts, budget, cancelled)?;
    for (group, target) in groups.iter().zip(&bound) {
        for update in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            let before = source
                .table
                .rows()
                .get(update.row())
                .and_then(|row| row.get(target.column))
                .ok_or(EditError::Conflict)?;
            if before != update.expected() {
                return Err(EditError::Conflict);
            }
        }
    }
    Ok(bound)
}

fn index_updates(
    source: &Source,
    groups: &[FixedColumnUpdates<'_>],
    bound: &[binding::Bound],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexKeyUpdate>, EditError> {
    let count = groups.iter().try_fold(0_usize, |count, group| {
        count
            .checked_add(group.updates().len())
            .ok_or(InspectError::StructureLimit)
    })?;
    budget
        .charge_allocation(count as u64, size_of::<IndexKeyUpdate>() as u64)
        .map_err(InspectError::from)?;
    let mut updates = Vec::new();
    updates
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for (group, target) in groups.iter().zip(bound) {
        let field = source
            .table
            .schema()
            .columns()
            .get(target.column)
            .ok_or(EditError::Preservation)?;
        let mut indexed = false;
        for index in source.indexes.indexes() {
            for key in index.columns() {
                check(budget, cancelled)?;
                indexed |= key.number() == field.number();
            }
        }
        if !indexed {
            continue;
        }
        let kind = MutationKey::new(ScalarType::Int32)?;
        for request in group.updates() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, COMPARISON_BYTES as u64)
                .map_err(InspectError::from)?;
            let before = cell(request.expected())?;
            let after = cell(request.value())?;
            if before == after {
                continue;
            }
            let record = source
                .records
                .get(request.row())
                .ok_or(EditError::Conflict)?;
            let expected = match before {
                Some(value) => Some(kind.stored(&value.to_le_bytes())?),
                None => None,
            };
            let value = match after {
                Some(value) => Some(kind.stored(&value.to_le_bytes())?),
                None => None,
            };
            updates.push(IndexKeyUpdate {
                reference: record.reference,
                column: target.column,
                expected,
                value,
            });
        }
    }
    Ok(updates)
}

fn cell(value: &Value) -> Result<Option<i32>, EditError> {
    match value {
        Value::Int32(value) => Ok(Some(*value)),
        Value::Null => Ok(None),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}

fn apply(
    output: &mut [u8],
    offset: usize,
    before: &[u8],
    replacement: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if before.len() != replacement.len() {
        return Err(EditError::Preservation);
    }
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    let destination = output.get_mut(offset..end).ok_or(EditError::Preservation)?;
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    if destination != before {
        return Err(EditError::Preservation);
    }
    destination.copy_from_slice(replacement);
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod resource_tests;

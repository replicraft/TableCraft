//! Independent original/final omission, root-leaf population and history proof.
use super::super::{population::Row, vector};
use super::{Columns, Operation, Population, keys};
use crate::{EditError, InspectError, WriteBlocker, header::check, inspect_table_index_counter};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Transition {
    before: Option<i32>,
    after: Option<i32>,
}

pub(super) fn reconstruct(
    operation: &Operation<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let index = operation.index;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if !constraints.ignore_nulls()
        || constraints.primary_key()
        || !operation.tree.root.children.is_empty()
        || operation.tree.root.prefix != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let schema = operation.checked.schema();
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    let native_position = schema
        .columns()
        .iter()
        .position(|field| field.number() == key.number())
        .ok_or(EditError::Preservation)?;
    let position = match operation.columns {
        Columns::Complete => native_position,
        Columns::Keys(positions) => {
            budget
                .charge(Resource::Steps, positions.len() as u64)
                .map_err(InspectError::from)?;
            positions
                .iter()
                .position(|position| *position == native_position)
                .ok_or(EditError::Preservation)?
        }
    };
    let Population::Nullable(rows) = &operation.population else {
        return Err(EditError::Preservation);
    };

    let mut transitions = vector(rows.len(), budget, cancelled)?;
    for row in *rows {
        check(budget, cancelled)?;
        let before = *row.before.get(position).ok_or(EditError::Preservation)?;
        let after = *row.after.get(position).ok_or(EditError::Preservation)?;
        // The field obligation precedes physical omission, for every alias.
        for alias in operation.checked.indexes() {
            check(budget, cancelled)?;
            if alias.index().data_number() == index.data_number() {
                alias
                    .index()
                    .constraints()
                    .ok_or(InspectError::Unsupported)?
                    .check_tuple::<i32>(&mut None, None, after.is_none())
                    .map_err(InspectError::from)?;
            }
        }
        transitions.push(Transition { before, after });
    }
    let columns = [(position, key.descending())];
    let original = emitted(rows, &columns, false, budget, cancelled)?;
    let final_keys = emitted(rows, &columns, true, budget, cancelled)?;
    operation.tree.original(&original, budget, cancelled)?;

    for alias in operation.checked.indexes() {
        check(budget, cancelled)?;
        if alias.index().data_number() != index.data_number() {
            continue;
        }
        let flags = alias
            .index()
            .constraints()
            .ok_or(InspectError::Unsupported)?;
        let mut previous = None;
        for entry in &final_keys {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, entry.len() as u64)
                .map_err(InspectError::from)?;
            let family = entry
                .get(
                    ..entry
                        .len()
                        .checked_sub(size_of::<u32>())
                        .ok_or(EditError::Preservation)?,
                )
                .ok_or(EditError::Preservation)?;
            flags
                .check_tuple(&mut previous, Some(family), false)
                .map_err(InspectError::from)?;
        }
    }
    let increment = historical_increment(&transitions, budget, cancelled)?;
    let mut comparison = original
        .len()
        .checked_mul(size_of::<Vec<u8>>())
        .ok_or(InspectError::StructureLimit)?;
    for entry in &original {
        check(budget, cancelled)?;
        comparison = comparison
            .checked_add(entry.len())
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge(Resource::Steps, comparison as u64)
        .map_err(InspectError::from)?;
    if original == final_keys {
        if increment != 0 {
            return Err(EditError::Preservation);
        }
        return super::super::replacement::reconstruct(
            operation.image,
            operation.tree,
            &original,
            &final_keys,
            expected,
            budget,
            cancelled,
        );
    }

    super::write_page(
        operation.image,
        &operation.tree.root,
        &final_keys,
        expected,
        budget,
        cancelled,
    )?;
    reconstruct_history(operation, increment, expected, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn emitted(
    rows: &[Row<Option<i32>>],
    columns: &[(usize, bool)],
    after: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<u8>>, EditError> {
    let complete = keys(rows, columns, after, budget, cancelled)?;
    let mut emitted = vector(complete.len(), budget, cancelled)?;
    for entry in complete {
        check(budget, cancelled)?;
        // Only the independently encoded present scalar has marker+I32+ROW.
        if entry.len() == size_of::<u8>() + size_of::<i32>() + size_of::<u32>() {
            emitted.push(entry);
        } else if entry.len() != size_of::<u8>() + size_of::<u32>() {
            return Err(EditError::Preservation);
        }
    }
    Ok(emitted)
}

fn historical_increment(
    transitions: &[Transition],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let mut introduced = vector(transitions.len(), budget, cancelled)?;
    for transition in transitions {
        check(budget, cancelled)?;
        let (None, Some(value)) = (transition.before, transition.after) else {
            continue;
        };
        let mut existed = false;
        let mut original_family_stays = false;
        let mut represented_replacement = false;
        for other in transitions {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (3 * size_of::<i32>()) as u64)
                .map_err(InspectError::from)?;
            existed |= other.before == Some(value);
            if other.before.is_some() && other.after == Some(value) {
                represented_replacement = true;
                original_family_stays |= other.before == Some(value);
            }
        }
        if existed && !original_family_stays || !existed && represented_replacement {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if existed {
            continue;
        }
        let work = introduced
            .len()
            .checked_mul(size_of::<i32>())
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        if !introduced.contains(&value) {
            introduced.push(value);
        }
    }
    Ok(introduced.len())
}

fn reconstruct_history(
    operation: &Operation<'_>,
    increment: usize,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if increment == 0 {
        return Ok(());
    }
    let schema = operation.checked.schema();
    let original = inspect_table_index_counter(
        &mut operation.image.reader(),
        operation.image.catalog().catalog().scope(),
        schema.name(),
        operation.index.name(),
        budget,
        cancelled,
    )?;
    let range = original.byte_range();
    let before = original.value().to_le_bytes();
    if original.definition_page() != schema.definition_page()
        || original.physical_index() != operation.index.data_number()
        || range.len() != size_of::<u32>()
        // Compare original and expected historical counter bytes while retaining range for the
        // returned counter patch. Only range bounds are copied, not the payload.
        || operation.image.as_bytes().get(range.clone()) != Some(before.as_slice())
        // Compare original and expected historical counter bytes while retaining range for the
        // returned counter patch. Only range bounds are copied, not the payload.
        || expected.get(range.clone()) != Some(before.as_slice())
    {
        return Err(EditError::Preservation);
    }
    let delta = u32::try_from(increment).map_err(|_| InspectError::StructureLimit)?;
    let after = original
        .value()
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge(Resource::Steps, (2 * size_of::<u32>()) as u64)
        .map_err(InspectError::from)?;
    expected
        .get_mut(range)
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&after.to_le_bytes());
    check(budget, cancelled)?;
    Ok(())
}

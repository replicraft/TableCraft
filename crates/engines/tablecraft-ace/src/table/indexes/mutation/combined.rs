//! One original-image physical transform for deletion and surviving NULL keys.
use super::{
    IndexKeyUpdate, PatchSet, delete, plan,
    profile::WriteProfile,
    tree::{Deletion, Tree},
};
use crate::format::reader::Reader;
use crate::table::indexes::{IndexCheck, NativeIndex, page::KeyRef};
use crate::{EditError, InspectError, NativeImage, TableSchema, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::table) fn index_delete_update_patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    deleted: &[u32],
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PatchSet, EditError> {
    for (position, reference) in deleted.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if deleted[..position].contains(reference) {
            return Err(EditError::Preservation);
        }
    }
    for (position, update) in updates.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                deleted.len().saturating_add(position) as u64,
            )
            .map_err(InspectError::from)?;
        if deleted.contains(&update.reference)
            || updates[..position].iter().any(|previous| {
                previous.reference == update.reference && previous.column == update.column
            })
        {
            return Err(EditError::Preservation);
        }
        let column = schema
            .columns()
            .get(update.column)
            .ok_or(EditError::Preservation)?;
        if update.expected.is_none()
            || update.value.is_some()
            || !matches!(
                column.scalar_type(),
                Some(
                    ScalarType::Byte
                        | ScalarType::Int16
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::Guid
                        | ScalarType::Decimal
                )
            )
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    plan::patches_with(
        image,
        schema,
        indexes,
        plan::Change::DeleteUpdate(deleted, updates),
        None,
        budget,
        cancelled,
    )
}

pub(super) fn edit<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    index: &NativeIndex,
    owner: u32,
    change: (&[u32], &[IndexKeyUpdate]),
    profile: &P,
    patches: &mut PatchSet,
) -> Result<(), EditError> {
    let (deleted, updates) = change;
    let mut keys = Vec::new();
    let tree = Tree::read::<P, N>(
        image,
        reader,
        owner,
        index.root_page(),
        profile,
        0,
        &mut keys,
    )?;
    let effective = delete::requests::capture(
        index,
        profile,
        keys.iter(),
        deleted,
        reader.budget,
        reader.cancelled,
    )?;
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let capacity = effective
        .references()
        .len()
        .checked_add(updates.len())
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(capacity as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut removed = Vec::new();
    removed
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    removed.extend_from_slice(effective.references());
    reader
        .budget
        .charge_allocation(keys.len() as u64, size_of::<KeyRef<P::Key>>() as u64)
        .map_err(InspectError::from)?;
    let mut survivors = Vec::new();
    survivors
        .try_reserve_exact(keys.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut matched = 0;
    for key in &mut keys {
        reader.tick()?;
        reader.work(
            updates
                .len()
                .saturating_add(removed.len())
                .saturating_add(profile.components().saturating_mul(2)),
        )?;
        if constraints.ignore_nulls() && profile.constraint_nulls(&key.key)?.0 {
            return Err(InspectError::Malformed.into());
        }
        for update in updates
            .iter()
            .filter(|update| update.reference == key.row && profile.includes(update.column))
        {
            profile.update(&mut key.key, update.column, update.expected, update.value)?;
            matched += 1;
        }
        if removed.contains(&key.row) {
            continue;
        }
        let (all_null, any_null) = profile.constraint_nulls(&key.key)?;
        // Required is checked BEFORE IgnoreNULL can omit a surviving key.
        let mut previous: Option<P::Key> = None;
        constraints
            .check_tuple(&mut previous, None, any_null)
            .map_err(InspectError::from)?;
        if constraints.ignore_nulls() && all_null {
            if removed.len() == capacity {
                return Err(EditError::Preservation);
            }
            removed.push(key.row);
        } else {
            survivors.push(*key);
        }
    }
    reader.work(updates.len())?;
    if matched
        != updates
            .iter()
            .filter(|update| profile.includes(update.column))
            .count()
    {
        return Err(EditError::Preservation);
    }
    let levels = survivors.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(
        survivors
            .len()
            .saturating_mul(levels)
            .saturating_mul(size_of::<KeyRef<P::Key>>()),
    )?;
    survivors.sort_unstable_by(|left, right| left.compare_with(right, profile));
    let mut previous = None;
    for key in &survivors {
        reader.tick()?;
        reader.work(profile.components())?;
        let (all_null, any_null) = profile.constraint_nulls(&key.key)?;
        constraints
            .check_tuple(
                &mut previous,
                if all_null { None } else { Some(key.key) },
                any_null,
            )
            .map_err(InspectError::from)?;
    }
    reader
        .budget
        .charge_allocation(keys.len() as u64, size_of::<KeyRef<P::Key>>() as u64)
        .map_err(InspectError::from)?;
    let mut partitioned = Vec::new();
    partitioned
        .try_reserve_exact(keys.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut original = keys.as_slice();
    let mut remaining = survivors.as_slice();
    tree.partition_survivors(
        &mut original,
        &mut remaining,
        &removed,
        &mut partitioned,
        reader.budget,
        reader.cancelled,
    )?;
    if !original.is_empty() || !remaining.is_empty() || partitioned.len() != keys.len() {
        return Err(EditError::Preservation);
    }
    delete::finish_tree::<P, N>(
        image,
        reader,
        index,
        &tree,
        &partitioned,
        Deletion {
            profile,
            references: &removed,
            removed: 0,
        },
        patches,
    )
}

#[cfg(test)]
mod tests;

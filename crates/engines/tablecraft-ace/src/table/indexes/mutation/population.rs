//! Original logical row associations certify complete final physical populations.
pub(super) mod history;
mod source;

use super::{IndexKeyUpdate, profile::WriteProfile, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::reader::Reader, table::indexes::page::KeyRef,
};
use std::io::Cursor;

pub(super) struct Prepared<K> {
    pub tree: Tree,
    pub entries: Vec<KeyRef<K>>,
    pub increment: usize,
    pub changed: bool,
}

pub(super) struct Transition<K> {
    before: K,
    after: K,
}

pub(super) fn prepare<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    schema: &TableSchema,
    updates: &[IndexKeyUpdate],
    profile: &P,
) -> Result<Prepared<P::Key>, EditError> {
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if constraints.ignore_nulls() && constraints.primary_key() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    for (position, update) in updates.iter().enumerate() {
        reader.tick()?;
        reader.work(position)?;
        if updates[..position]
            .iter()
            .any(|prior| prior.reference == update.reference && prior.column == update.column)
        {
            return Err(EditError::Conflict);
        }
    }

    let mut original = Vec::new();
    let tree = Tree::read::<_, N>(
        image,
        reader,
        schema.definition_page(),
        index.root_page(),
        profile,
        0,
        &mut original,
    )?;
    let all_rows = source::capture(image, schema, index, profile, reader)?;
    let mut eligible = Vec::new();
    reserve(&mut eligible, all_rows.len(), reader)?;
    for key in &all_rows {
        reader.tick()?;
        if !constraints.ignore_nulls() || !profile.constraint_nulls(&key.key)?.0 {
            eligible.push(*key);
        }
    }
    sort(&mut eligible, profile, reader)?;
    if !same(&original, &eligible, profile, reader)? {
        return Err(EditError::Preservation);
    }

    let mut final_keys = Vec::new();
    let mut transitions = Vec::new();
    reserve(&mut final_keys, all_rows.len(), reader)?;
    reserve(&mut transitions, all_rows.len(), reader)?;
    let mut matched = 0;
    for entry in all_rows {
        reader.tick()?;
        reader.work(updates.len().saturating_mul(profile.components()))?;
        let before = entry.key;
        let mut after = before;
        for update in updates
            .iter()
            .filter(|update| update.reference == entry.row && profile.includes(update.column))
        {
            profile.update(&mut after, update.column, update.expected, update.value)?;
            matched += 1;
        }
        let (all_null, any_null) = profile.constraint_nulls(&after)?;
        // Required is a field obligation even when the resulting key is omitted.
        constraints
            .check_tuple::<P::Key>(&mut None, None, any_null)
            .map_err(InspectError::from)?;
        if !constraints.ignore_nulls() || !all_null {
            final_keys.push(KeyRef {
                key: after,
                row: entry.row,
            });
        }
        transitions.push(Transition { before, after });
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
    sort(&mut final_keys, profile, reader)?;
    let mut previous = None;
    for entry in &final_keys {
        reader.tick()?;
        reader.work(profile.max_key_bytes())?;
        let (_, any_null) = profile.constraint_nulls(&entry.key)?;
        constraints
            .check_tuple(
                &mut previous,
                if profile.constraint_nulls(&entry.key)?.0 {
                    None
                } else {
                    Some(entry.key)
                },
                any_null,
            )
            .map_err(InspectError::from)?;
    }
    let increment = if constraints.ignore_nulls() {
        history::increment(&transitions, &original, profile, reader)?
    } else {
        0
    };
    let changed = !same(&original, &final_keys, profile, reader)?;
    if !changed && increment != 0 {
        return Err(EditError::Preservation);
    }
    Ok(Prepared {
        tree,
        entries: final_keys,
        increment,
        changed,
    })
}

fn sort<P: WriteProfile>(
    keys: &mut [KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = keys
        .len()
        .checked_mul(levels)
        .and_then(|count| count.checked_mul(profile.max_key_bytes()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    keys.sort_unstable_by(|left, right| left.compare_with(right, profile));
    Ok(())
}

fn same<P: WriteProfile>(
    left: &[KeyRef<P::Key>],
    right: &[KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(right) {
        reader.tick()?;
        reader.work(profile.max_key_bytes())?;
        if left.row != right.row || !profile.compare(&left.key, &right.key).is_eq() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}

//! Complete strict-key sort and checked source/display permutations.
use super::super::{BoundSort, DisplayOrder, FilterError, SortSpec, criteria};
use crate::{
    Error,
    controls::{SCOPE, budget, check, comparison_budget},
};
use std::{cell::RefCell, cmp::Ordering};
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{NativeReadSource, inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_query::keys::{compare_stored_keys, validate_stored_key};
use tablecraft_schema::{ScalarType, TypeDescriptor};

const KEY_BYTES: u64 = 64 * 1024 * 1024;
const KEY_STEPS: u64 = 10_000_000;
const KEY_BATCH_ROWS: u32 = 100;
const INITIAL_KEYS: usize = 64;

pub(super) struct SortInput<'a> {
    pub(super) table: &'a str,
    pub(super) spec: &'a SortSpec,
    pub(super) expected: Option<TypeDescriptor>,
    pub(super) members: &'a [u32],
    pub(super) source_rows: u32,
    pub(super) enabled: bool,
}

pub(super) fn build<R: NativeReadSource>(
    source: &mut R,
    input: SortInput<'_>,
    retained: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(BoundSort, DisplayOrder), Error> {
    let mut native = budget();
    let schema = inspect_table_schema(source, SCOPE, input.table, &mut native, cancelled)?;
    let selected = criteria::field_position(&schema, &input.spec.column, &mut native, cancelled)?;
    criteria::require_field(&schema.columns()[selected])?;
    if matches!(
        schema.columns()[selected].scalar_type(),
        Some(ScalarType::Binary | ScalarType::Guid) | None
    ) {
        return Err(FilterError::UnsupportedField.into());
    }
    let database_order = inspect_database_sort_order(source, &mut native, cancelled)?;
    let comparisons = RefCell::new(comparison_budget());
    let resolver =
        NativeCollationResolver::new_deferred_default(database_order, &comparisons, cancelled)?;
    let field = &schema.columns()[selected];
    let policy = if field.scalar_type() == Some(ScalarType::Text) {
        Some(resolver.resolve_native(field.text_sort_order().unwrap_or(database_order))?)
    } else {
        None
    };

    let mut browser = NativeBrowser::new(source, SCOPE, native, cancelled);
    let mut cursor = browser.projected_cursor(input.table, &[&input.spec.column])?;
    let declaration = cursor
        .columns()
        .first()
        .ok_or(FilterError::UnsupportedField)?
        .type_descriptor();
    if input
        .expected
        .is_some_and(|expected| expected != declaration)
    {
        return Err(FilterError::UnsupportedField.into());
    }
    let mut work = ResourceBudget::new(KEY_BYTES, 0, KEY_STEPS);
    let mut keys = Vec::new();
    let mut key_ordinals = Vec::new();
    let mut seen = 0_u32;
    let mut next_member = 0;
    loop {
        let batch = cursor.next_page(KEY_BATCH_ROWS)?;
        if batch.offset() != seen {
            return Err(FilterError::InvalidSelection.into());
        }
        for row in batch.rows() {
            check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            let ordinal = seen;
            seen = seen.checked_add(1).ok_or(FilterError::InvalidSelection)?;
            if input.enabled {
                if input.members.get(next_member).copied() != Some(ordinal) {
                    continue;
                }
                next_member += 1;
            }
            let value = row.first().ok_or(FilterError::UnsupportedField)?;
            validate_stored_key(value)?;
            if keys.len() == keys.capacity() {
                let additional = keys.capacity().max(INITIAL_KEYS);
                work.charge_allocation(additional as u64, size_of::<Value>() as u64)?;
                keys.try_reserve_exact(additional)?;
            }
            if key_ordinals.len() == key_ordinals.capacity() {
                let additional = key_ordinals.capacity().max(INITIAL_KEYS);
                work.charge_allocation(additional as u64, size_of::<u32>() as u64)?;
                key_ordinals.try_reserve_exact(additional)?;
            }
            keys.push(criteria::copy_value(value, &mut work, cancelled)?);
            key_ordinals.push(ordinal);
        }
        if batch.reached_end() {
            break;
        }
    }
    if seen != input.source_rows || (input.enabled && next_member != input.members.len()) {
        return Err(FilterError::InvalidSelection.into());
    }

    let length = keys.len();
    work.charge_allocation(length as u64, size_of::<u32>() as u64)?;
    let mut indices = Vec::new();
    indices.try_reserve_exact(length)?;
    indices.extend(0..u32::try_from(length).map_err(|_| FilterError::InvalidSelection)?);
    work.charge_allocation(length as u64, size_of::<u32>() as u64)?;
    let mut scratch = Vec::new();
    scratch.try_reserve_exact(length)?;
    scratch.resize(length, 0);
    let mut width = 1;
    while width < length {
        let mut start = 0;
        while start < length {
            let middle = start.saturating_add(width).min(length);
            let end = middle.saturating_add(width).min(length);
            let (mut left, mut right) = (start, middle);
            for destination in &mut scratch[start..end] {
                check(cancelled)?;
                work.charge(Resource::Steps, 1)?;
                let take_left = if left == middle {
                    false
                } else if right == end {
                    true
                } else {
                    work.charge(Resource::Steps, 1)?;
                    let mut order = compare_stored_keys(
                        &keys[indices[left] as usize],
                        &keys[indices[right] as usize],
                        policy,
                    )?;
                    if input.spec.descending {
                        order = order.reverse();
                    }
                    order != Ordering::Greater
                };
                if take_left {
                    *destination = indices[left];
                    left += 1;
                } else {
                    *destination = indices[right];
                    right += 1;
                }
            }
            start = end;
        }
        std::mem::swap(&mut indices, &mut scratch);
        width = width.saturating_mul(2);
    }

    for index in &mut indices {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        *index = key_ordinals[*index as usize];
    }

    // Transfer the complete active-mode permutation into the retained handle budget.
    retained.charge_allocation(length as u64, size_of::<u32>() as u64)?;
    let order = index_maps(
        indices,
        input.members,
        input.source_rows,
        input.enabled,
        retained,
        &mut work,
        cancelled,
    )?;
    let bound = BoundSort {
        spec: SortSpec {
            column: criteria::copy_text(&input.spec.column, retained, cancelled)?,
            descending: input.spec.descending,
        },
        declaration,
    };
    Ok((bound, order))
}

fn index_maps(
    ordinals: Vec<u32>,
    members: &[u32],
    source_rows: u32,
    enabled: bool,
    retained: &mut ResourceBudget,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DisplayOrder, Error> {
    let length = source_rows as usize;
    retained.charge_allocation(length as u64, size_of::<Option<u32>>() as u64)?;
    let mut ranks = Vec::new();
    ranks.try_reserve_exact(length)?;
    ranks.resize(length, None);
    for (position, &ordinal) in ordinals.iter().enumerate() {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        if enabled {
            work.charge(Resource::Steps, u64::from(members.len().ilog2() + 1))?;
            if members.binary_search(&ordinal).is_err() {
                return Err(FilterError::InvalidSelection.into());
            }
        }
        let slot = ranks
            .get_mut(ordinal as usize)
            .ok_or(FilterError::InvalidSelection)?;
        if slot.is_some() {
            return Err(FilterError::InvalidSelection.into());
        }
        *slot = Some(u32::try_from(position).map_err(|_| FilterError::InvalidSelection)?);
    }
    let expected = if enabled { members.len() } else { length };
    if ordinals.len() != expected {
        return Err(FilterError::InvalidSelection.into());
    }
    Ok(DisplayOrder {
        enabled,
        ordinals,
        ranks,
    })
}

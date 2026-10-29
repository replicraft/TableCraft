//! One original table, multiple physical owners, one qualified append ledger.
mod census;
mod prepared;

use super::{AllocationPolicy, Plan, seal};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, TableCheck, TableSchema, WriteBlocker,
    format::reader::Reader,
    header::check,
    table::{
        indexes::IndexKeyUpdate,
        mutation::{
            append::{State, owners::MultipleOwners},
            map_authority::Owner,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct Input<'a> {
    pub checked: &'a TableCheck,
    pub updates: &'a [IndexKeyUpdate],
}

/// Every source population and role is certified before the common cursor exists.
pub(in crate::table) fn multiple(
    image: &NativeImage,
    inputs: &[Input<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan<MultipleOwners>, EditError> {
    check(budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut ordered = Vec::new();
    reserve(&mut ordered, inputs.len(), &mut reader)?;
    ordered.extend(inputs.iter());
    reader.work(inputs.len().saturating_mul(inputs.len()))?;
    ordered.sort_unstable_by_key(|input| input.checked.schema().definition_page());
    for pair in ordered.windows(2) {
        reader.tick()?;
        if pair[0].checked.schema().definition_page() == pair[1].checked.schema().definition_page()
        {
            return Err(InspectError::Malformed.into());
        }
    }

    let mut tables = Vec::new();
    let mut checked_indexes = Vec::new();
    let mut prepared = Vec::new();
    let mut pages = Vec::new();
    reserve(&mut tables, ordered.len(), &mut reader)?;
    for input in ordered {
        reader.tick()?;
        let schema = input.checked.schema();
        tables.push(input.checked);
        let owners = census::capture(schema, input.checked.indexes(), &mut reader)?;
        reserve(
            &mut checked_indexes,
            input.checked.indexes().len(),
            &mut reader,
        )?;
        checked_indexes.extend(input.checked.indexes().iter());
        for owner in owners {
            reader.tick()?;
            for alias in input
                .checked
                .indexes()
                .iter()
                .filter(|alias| alias.index().data_number() == owner.index.data_number())
            {
                reader.tick()?;
                let expected = owner
                    .index
                    .constraints()
                    .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                let actual = alias
                    .index()
                    .constraints()
                    .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
                if actual.unique() != expected.unique()
                    || actual.required() != expected.required()
                    || actual.ignore_nulls() != expected.ignore_nulls()
                    || alias.index().columns().len() != owner.index.columns().len()
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                reader.work(owner.index.columns().len())?;
                if alias
                    .index()
                    .columns()
                    .iter()
                    .zip(owner.index.columns())
                    .any(|(left, right)| {
                        left.number() != right.number() || left.descending() != right.descending()
                    })
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
            if owner
                .index
                .constraints()
                .is_some_and(|constraints| constraints.ignore_nulls())
            {
                reader.work(input.checked.indexes().len())?;
                if input.checked.indexes().iter().any(|alias| {
                    alias.index().data_number() == owner.index.data_number()
                        && alias
                            .index()
                            .constraints()
                            .is_some_and(|constraints| constraints.primary_key())
                }) {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
            }
            for &page in &owner.pages {
                reader.tick()?;
                reader.work(pages.len())?;
                if pages.contains(&page) {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                reserve(&mut pages, 1, &mut reader)?;
                pages.push(page);
            }
            // Complete traversal certifies unaffected owners as well. The multi-table
            // allocation profile currently uses Int32 single/tuple owners throughout.
            if let Some(index) =
                prepared::prepare(image, schema, &owner, input.updates, true, &mut reader)?
            {
                reserve(&mut prepared, 1, &mut reader)?;
                prepared.push((schema, index));
            }
        }
    }
    let context = MultipleOwners::capture(image, &tables, &pages, reader.budget, reader.cancelled)?;
    let append = State::with_owner(image, context)?;
    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    for (schema, index) in prepared {
        index.write(image, schema, &mut plan, &mut reader)?;
    }
    seal_multiple(image, &checked_indexes, &mut plan, &mut reader)?;
    check(reader.budget, reader.cancelled)?;
    Ok(plan)
}

fn seal_multiple(
    image: &NativeImage,
    indexes: &[&IndexCheck],
    plan: &mut Plan<MultipleOwners>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    super::allocation::seal_references(image, indexes, plan, reader)
}

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let owners = census::capture(schema, indexes, &mut reader)?;
    let mut prepared = Vec::new();
    reserve(&mut prepared, owners.len(), &mut reader)?;
    for owner in &owners {
        reader.tick()?;
        if owner
            .index
            .constraints()
            .is_some_and(|constraints| constraints.ignore_nulls())
        {
            reader.work(indexes.len())?;
            if indexes.iter().any(|alias| {
                alias.index().data_number() == owner.index.data_number()
                    && alias
                        .index()
                        .constraints()
                        .is_some_and(|constraints| constraints.primary_key())
            }) {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        if let Some(index) = prepared::prepare(image, schema, owner, updates, false, &mut reader)? {
            prepared.push(index);
        }
    }
    // All source tree/profile/constraint and cross-owner checks precede State
    // reservations. No prior candidate supplies ownership or eligible rows.
    let append = State::new(image, schema.definition_page());
    let mut roles = Vec::new();
    reserve(&mut roles, owners.len() + 3, &mut reader)?;
    roles.extend([
        Owner::GlobalFree,
        Owner::OrdinaryOwned {
            definition: schema.definition_page(),
        },
        Owner::OrdinaryCandidate {
            definition: schema.definition_page(),
        },
    ]);
    for owner in &owners {
        roles.push(Owner::IndexOwned {
            definition: schema.definition_page(),
            physical: owner.index.data_number(),
        });
    }
    append.require_source_map_exclusive(image, &roles, &mut reader)?;
    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    for index in prepared {
        index.write(image, schema, &mut plan, &mut reader)?;
    }
    seal(image, indexes, &mut plan, reader.budget, reader.cancelled)?;
    check(reader.budget, reader.cancelled)?;
    Ok(plan)
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
        .try_reserve(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}

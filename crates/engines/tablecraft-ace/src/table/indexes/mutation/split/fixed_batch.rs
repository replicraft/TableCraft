//! One original table, multiple physical owners, one qualified append ledger.
mod census;
mod prepared;

use super::{AllocationPolicy, Plan, seal};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::reader::Reader,
    header::check,
    table::{
        indexes::IndexKeyUpdate,
        mutation::{append::State, map_authority::Owner},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

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
        if let Some(index) = prepared::prepare(image, schema, owner, updates, &mut reader)? {
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

//! Joint final rows continue the existing DATA append cursor exactly once.
mod typed;

use super::super::replacement::{self, RowKeyUpdate};
use super::{AllocationPolicy, Plan, seal, source};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, TableSchema,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::mutation::{append::State, map_authority::Owner},
};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct Changes<'a> {
    pub columns: &'a [usize],
    pub updates: &'a [RowKeyUpdate<'a>],
}

pub(in crate::table) fn continue_patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    changes: &Changes<'_>,
    append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    replacement::qualify_row_indexes(schema, indexes, changes.columns, budget, cancelled)?;
    if append.original_length != image.as_bytes().len()
        || append.owner != schema.definition_page()
        || append.length < append.original_length
        || !append.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    append.preview(0)?;

    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    replacement::rows::validate(schema, changes.updates, &mut reader)?;
    let owners = source::capture(schema, indexes, &mut reader)?;
    reader
        .budget
        .charge_allocation(owners.len() as u64, size_of::<typed::Prepared<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(owners.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut roles = Vec::new();
    reader
        .budget
        .charge_allocation((owners.len() + 3) as u64, size_of::<Owner>() as u64)
        .map_err(InspectError::from)?;
    roles
        .try_reserve_exact(owners.len() + 3)
        .map_err(|_| InspectError::Allocation)?;
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
        reader.tick()?;
        let comparisons = owner
            .index
            .columns()
            .len()
            .checked_mul(changes.columns.len())
            .ok_or(InspectError::Malformed)?;
        reader.work(comparisons)?;
        let affected = !changes.updates.is_empty()
            && replacement::mixed::qualification::affects(schema, owner.index, changes.columns)?;
        let updates = if affected { changes.updates } else { &[] };
        let typed = typed::prepare(image, schema, owner, updates, &mut reader)?;
        if affected {
            prepared.push(typed);
            roles.push(Owner::IndexOwned {
                definition: schema.definition_page(),
                physical: owner.index.data_number(),
            });
        }
    }
    append.require_source_map_exclusive(image, &roles, &mut reader)?;

    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    for owner in prepared {
        owner.write(image, &mut plan, &mut reader)?;
    }
    seal(image, indexes, &mut plan, reader.budget, reader.cancelled)?;
    check(reader.budget, reader.cancelled)?;
    Ok(plan)
}

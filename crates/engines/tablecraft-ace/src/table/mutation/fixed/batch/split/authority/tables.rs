//! Requalify original table identities and derive their field facts independently.
use super::{cells, push};
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage, TableCheck, header::check,
    table::mutation::gates,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Table {
    pub checked: TableCheck,
    pub fields: cells::Fields,
}

pub(super) fn derive(
    image: &NativeImage,
    supplied: &[&TableCheck],
    groups: &[FixedColumnUpdates<'_>],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Table>, EditError> {
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut assignments = 0usize;
    let mut originals: Vec<TableCheck> = Vec::new();
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        assignments = assignments
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, (position + originals.len()) as u64)
            .map_err(InspectError::from)?;
        if groups[..position].iter().any(|prior| {
            prior.table().eq_ignore_ascii_case(group.table())
                && prior.column().eq_ignore_ascii_case(group.column())
        }) {
            return Err(EditError::Conflict);
        }
        if originals
            .iter()
            .any(|table| table.schema().name().eq_ignore_ascii_case(group.table()))
        {
            continue;
        }
        let original = gates::before_fixed_update(image, group.table(), budget, cancelled)?;
        if originals.iter().any(|prior| {
            prior.schema().definition_page() == original.schema().definition_page()
                || prior.schema().object_id() == original.schema().object_id()
        }) {
            return Err(EditError::Preservation);
        }
        push(&mut originals, original, budget)?;
    }
    if originals.len() != supplied.len() {
        return Err(EditError::Preservation);
    }
    for (position, candidate) in supplied.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (position + originals.len()) as u64)
            .map_err(InspectError::from)?;
        if supplied[..position]
            .iter()
            .any(|prior| prior.schema().definition_page() == candidate.schema().definition_page())
            || !originals.iter().any(|original| {
                original.schema().same_definition(candidate.schema())
                    && original.schema().object_id() == candidate.schema().object_id()
            })
        {
            return Err(EditError::Preservation);
        }
    }
    let levels = originals.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(
            Resource::Steps,
            originals.len().saturating_mul(levels) as u64,
        )
        .map_err(InspectError::from)?;
    originals.sort_unstable_by_key(|table| table.schema().definition_page());

    let mut result = Vec::new();
    for checked in originals {
        let mut selected = Vec::new();
        for group in groups {
            check(budget, cancelled)?;
            if group.table().eq_ignore_ascii_case(checked.schema().name()) {
                push(
                    &mut selected,
                    FixedColumnUpdates::new(group.table(), group.column(), group.updates()),
                    budget,
                )?;
            }
        }
        let fields = cells::derive(image, checked.schema(), &selected, after, budget, cancelled)?;
        push(&mut result, Table { checked, fields }, budget)?;
    }
    Ok(result)
}

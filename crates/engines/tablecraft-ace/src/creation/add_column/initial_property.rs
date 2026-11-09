//! Initial external-property image application shared by qualified field owners.
use super::super::{CreateDatabaseError, record::DataPage};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::catalog_property::{self, Change},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const LVAL_OWNER: usize = u32::from_le_bytes(*b"LVAL") as usize;

#[derive(Debug, Error)]
pub(super) enum InitialPropertyError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    #[error(transparent)]
    Placement(#[from] EditError),
    #[error("initial property image preservation check failed")]
    Preservation,
}

pub(super) fn apply(
    before: &NativeImage,
    schema: &TableSchema,
    mut after: Vec<u8>,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Change), InitialPropertyError> {
    let change = catalog_property::compose(before, schema, body, budget, cancelled)?;
    budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    after
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    let original_length = after.len();
    after.resize(original_length + PAGE_BYTES, 0);
    let mut page = DataPage::new(&mut after[original_length..], LVAL_OWNER)?;
    page.append(change.body.len(), budget, cancelled)?
        .copy_from_slice(&change.body);
    page.finish()?;
    check(budget, cancelled)?;
    let offset = change.page as usize * PAGE_BYTES;
    after[offset..offset + PAGE_BYTES].copy_from_slice(&change.bytes);
    for cell in &change.maps {
        let byte = after
            .get_mut(cell.offset)
            .ok_or(InitialPropertyError::Preservation)?;
        if *byte != cell.before {
            return Err(InitialPropertyError::Preservation);
        }
        *byte |= cell.mask;
    }
    Ok((after, change))
}

//! Compose through existing declaration, MR2, placement and DATA-page owners.
use super::CreatedDefaultZeroAddColumnError as Error;
use crate::{
    CreateColumn, InspectError, NativeImage, TableCheck,
    creation::{
        input::{Columns, Rows},
        properties,
        record::DataPage,
    },
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::catalog_property::{self, Change},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const LVAL_OWNER: usize = u32::from_le_bytes(*b"LVAL") as usize;

pub(super) fn properties(
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let prepared = properties::capture(
        Columns::Values(columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?
    .ok_or(Error::Preservation)?;
    let length = prepared.inline_length();
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(length, 0);
    prepared.write_inline(&mut bytes, budget, cancelled)?;
    bytes.drain(..DESCRIPTOR_BYTES);
    Ok(bytes)
}

pub(super) fn candidate(
    before: &NativeImage,
    checked: &TableCheck,
    mut after: Vec<u8>,
    body: Vec<u8>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Change), Error> {
    let change = catalog_property::compose(before, checked.schema(), body, budget, cancelled)?;
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
        let byte = after.get_mut(cell.offset).ok_or(Error::Preservation)?;
        if *byte != cell.before {
            return Err(Error::Preservation);
        }
        *byte |= cell.mask;
    }
    Ok((after, change))
}

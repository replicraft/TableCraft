//! Original root metadata chooses the qualified fitting attempt, never a fallback.
use crate::{
    EditError, InspectError, NativeImage,
    format::page::PAGE_BYTES,
    header::check,
    table::{
        indexes::{header::IndexHeader, page::NODE},
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn selected(
    image: &NativeImage,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    for index in source.indexes.indexes() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, size_of::<IndexHeader>() as u64)
            .map_err(InspectError::from)?;
        let start = (index.root_page() as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        let raw = image
            .as_bytes()
            .get(start..end)
            .ok_or(EditError::Preservation)?;
        if IndexHeader::read(raw)?.kind == NODE {
            return Ok(true);
        }
    }
    Ok(false)
}

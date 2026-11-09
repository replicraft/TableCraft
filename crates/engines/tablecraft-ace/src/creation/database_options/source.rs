//! The exact existing OFF source proof, shared with inert review availability.
use super::{
    DATABASE_ID, DatabaseOptionsError, MAP_BYTES, MAP_PAGE, MAP_PAGE_CAPACITY, ORIGINAL_OBJECTS,
    PROPERTY_CANDIDATE, PROPERTY_OWNED, source_error,
};
use crate::{NativeImage, creation::add_table::source, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn capture(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(source::Source, usize), DatabaseOptionsError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().catalog().objects().len() != ORIGINAL_OBJECTS {
        return Err(DatabaseOptionsError::Source);
    }
    source::require_name(image, "MSysDb", budget, cancelled).map_err(source_error)?;
    for origin in image.catalog().origins() {
        check(budget, cancelled)?;
        if origin.native_id() == DATABASE_ID as i32 {
            return Err(DatabaseOptionsError::Source);
        }
    }
    let (geometry, legacy) =
        source::derive_geometry(image, false, budget, cancelled).map_err(source_error)?;
    let original = source::qualify_geometry(image, geometry, legacy, budget, cancelled)
        .map_err(source_error)?;
    let new_page = image.as_bytes().len() / PAGE_BYTES;
    if new_page >= MAP_PAGE_CAPACITY {
        return Err(DatabaseOptionsError::Capacity);
    }
    for slot in [PROPERTY_OWNED, PROPERTY_CANDIDATE] {
        let range = original
            .maps
            .records
            .get(slot)
            .ok_or(DatabaseOptionsError::Source)?;
        if image.as_bytes()[MAP_PAGE * PAGE_BYTES + range.start..MAP_PAGE * PAGE_BYTES + range.end]
            != [0; MAP_BYTES]
        {
            return Err(DatabaseOptionsError::Source);
        }
    }

    Ok((original, new_page))
}

//! Advisory source facts use the actual operation's complete native qualification.
use super::{AddTableError, geometry::MAPPED_PAGES, retained, source};
use crate::format::page::PAGE_BYTES;
use crate::{InspectError, NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const KEYLESS_PAGES: usize = 2;
const KEYED_PAGES: usize = 3;
const MAP_RECORD_BYTES: usize = 13;
const PAGE_HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();
const MIN_SOURCE_PAGES: usize = 9;
const MAX_SOURCE_PAGES: usize = MAPPED_PAGES - KEYLESS_PAGES;

/// Target-specific page/map capacity of a freshly qualified structural source.
/// These advisory facts do not authorize a particular name or declaration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddTableSourceCapabilities {
    pub(super) keyless: bool,
    pub(super) keyed: bool,
}

impl AddTableSourceCapabilities {
    /// Whether the qualified source can accommodate a two-page keyless table.
    pub const fn can_add_keyless(self) -> bool {
        self.keyless
    }
    /// Whether the qualified source can accommodate a three-page keyed table.
    pub const fn can_add_keyed(self) -> bool {
        self.keyed
    }
}

/// Preserve the existing generated-table advisory boundary (three new pages).
pub fn add_table_source_available(
    captured: &[u8],
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    Ok(add_table_source_capabilities(captured, scope, budget, cancelled)?.can_add_keyed())
}

/// Qualify immutable source roles, keeping two-page keyless and three-page keyed
/// capacity separate. Unsupported lengths refuse before capture/allocation.
/// Malformed or unsupported sources return unavailable facts; cancellation,
/// resources and host errors remain errors rather than silent unavailability.
pub fn add_table_source_capabilities(
    captured: &[u8],
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTableSourceCapabilities, AddTableError> {
    let _span = tracing::debug_span!("native.add_table.source_available").entered();
    check(budget, cancelled)?;
    if !captured.len().is_multiple_of(PAGE_BYTES) {
        return Ok(AddTableSourceCapabilities::default());
    }
    let result = (|| {
        let image = NativeImage::capture_for_table_append(
            &mut Cursor::new(captured),
            scope,
            budget,
            cancelled,
        )?;
        if retained::selected(&image, budget, cancelled)? {
            return retained::capabilities(&image, budget, cancelled);
        }
        if !(MIN_SOURCE_PAGES..=MAX_SOURCE_PAGES).contains(&(captured.len() / PAGE_BYTES)) {
            return Ok(AddTableSourceCapabilities::default());
        }
        let (geometry, legacy) = source::derive_geometry(&image, false, budget, cancelled)?;
        let qualified = if legacy {
            source::qualify(&image, budget, cancelled)?
        } else {
            source::qualify_geometry(&image, geometry, false, budget, cancelled)?
        };
        let fits_maps = |count| {
            qualified.maps.start
                >= count * MAP_RECORD_BYTES
                    + PAGE_HEADER_BYTES
                    + (qualified.maps.count + count) * SLOT_BYTES
        };
        Ok(AddTableSourceCapabilities {
            keyless: geometry.before_pages + KEYLESS_PAGES <= MAPPED_PAGES
                && fits_maps(KEYLESS_PAGES),
            keyed: geometry.before_pages + KEYED_PAGES <= MAPPED_PAGES && fits_maps(KEYED_PAGES),
        })
    })();
    match result {
        Ok(capabilities) => Ok(capabilities),
        Err(AddTableError::Source | AddTableError::Capacity) => {
            Ok(AddTableSourceCapabilities::default())
        }
        Err(AddTableError::Inspect(
            InspectError::Unsupported
            | InspectError::Malformed
            | InspectError::Truncated
            | InspectError::UnknownTable
            | InspectError::UnknownColumn
            | InspectError::UnknownObject
            | InspectError::UnknownIndex
            | InspectError::IndexConstraint(_)
            | InspectError::Constraint(_)
            | InspectError::ComplexReference(_)
            | InspectError::Model(_)
            | InspectError::Schema(_),
        )) => Ok(AddTableSourceCapabilities::default()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;

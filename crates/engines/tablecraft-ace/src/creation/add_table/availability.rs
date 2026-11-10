//! Advisory source facts use the actual operation's complete native qualification.
use super::{AddTableError, geometry::MAPPED_PAGES, native, retained, source};
use crate::format::page::PAGE_BYTES;
use crate::{Header, InspectError, NativeImage, header::check};
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

/// Independently qualified original/native construction capacity. This is
/// source advice only; named consuming plans still bind their complete request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredAddTableSourceCapabilities {
    capabilities: AddTableSourceCapabilities,
    native_required: bool,
}

impl AuthoredAddTableSourceCapabilities {
    /// Some minimal keyless declaration fits the qualified source storage.
    pub const fn can_add_keyless(self) -> bool {
        self.capabilities.keyless
    }

    /// Some minimal keyed declaration fits the qualified source storage.
    pub const fn can_add_keyed(self) -> bool {
        self.capabilities.keyed
    }

    /// At least one advertised shape needs native-count construction/source
    /// admission. Prefer Original only when it covers every advertised fact.
    pub const fn requires_native(self) -> bool {
        self.native_required
    }
}

/// Qualify original and native source capabilities independently, selecting no
/// action decoder and retrying no mutation. Existing-source records may be read
/// to prove ownership; they never invent the proposed declaration or its schema.
pub fn authored_add_table_source_capabilities(
    captured: &[u8],
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AuthoredAddTableSourceCapabilities, AddTableError> {
    let _span = tracing::debug_span!("native.add_table.authored_source_available").entered();
    check(budget, cancelled)?;
    if !captured.len().is_multiple_of(PAGE_BYTES)
        || Header::parse(captured, captured.len() as u64).is_err()
    {
        return Ok(AuthoredAddTableSourceCapabilities::default());
    }
    source_result((|| {
        let image =
            NativeImage::capture_retained(&mut Cursor::new(captured), scope, budget, cancelled)?;
        let original = source_result(original_capabilities(&image, budget, cancelled))?;
        let native = source_result(native::capabilities(&image, budget, cancelled))?;
        Ok(AuthoredAddTableSourceCapabilities {
            capabilities: AddTableSourceCapabilities {
                keyless: original.keyless || native.keyless,
                keyed: original.keyed || native.keyed,
            },
            native_required: native.keyless && !original.keyless || native.keyed && !original.keyed,
        })
    })())
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
/// capacity separate. Invalid framing refuses before capture/allocation; source
/// dialect and placement capacity require native metadata discovery.
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
    if !captured.len().is_multiple_of(PAGE_BYTES)
        || Header::parse(captured, captured.len() as u64).is_err()
    {
        return Ok(AddTableSourceCapabilities::default());
    }
    let result = (|| {
        let image =
            NativeImage::capture_retained(&mut Cursor::new(captured), scope, budget, cancelled)?;
        original_capabilities(&image, budget, cancelled)
    })();
    source_result(result)
}

pub(super) fn original_source_supports(
    image: &NativeImage,
    keyed: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    let capabilities = source_result(original_capabilities(image, budget, cancelled))?;
    Ok(if keyed {
        capabilities.keyed
    } else {
        capabilities.keyless
    })
}

fn original_capabilities(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTableSourceCapabilities, AddTableError> {
    if retained::selected(image, budget, cancelled)? {
        return retained::capabilities(image, budget, cancelled);
    }
    if !(MIN_SOURCE_PAGES..=MAX_SOURCE_PAGES).contains(&(image.as_bytes().len() / PAGE_BYTES)) {
        return Ok(AddTableSourceCapabilities::default());
    }
    let (geometry, legacy) = source::derive_geometry(image, false, budget, cancelled)?;
    let qualified = if legacy {
        source::qualify(image, budget, cancelled)?
    } else {
        source::qualify_geometry(image, geometry, false, budget, cancelled)?
    };
    let fits_maps = |count| {
        qualified.maps.start
            >= count * MAP_RECORD_BYTES
                + PAGE_HEADER_BYTES
                + (qualified.maps.count + count) * SLOT_BYTES
    };
    Ok(AddTableSourceCapabilities {
        keyless: geometry.before_pages + KEYLESS_PAGES <= MAPPED_PAGES && fits_maps(KEYLESS_PAGES),
        keyed: geometry.before_pages + KEYED_PAGES <= MAPPED_PAGES && fits_maps(KEYED_PAGES),
    })
}

fn source_result<T: Default>(result: Result<T, AddTableError>) -> Result<T, AddTableError> {
    match result {
        Ok(capabilities) => Ok(capabilities),
        Err(AddTableError::Source | AddTableError::Capacity) => Ok(T::default()),
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
        )) => Ok(T::default()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;

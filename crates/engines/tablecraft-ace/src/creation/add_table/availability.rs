//! Advisory source facts use the actual operation's complete native qualification.
use super::{AddTableError, source};
use crate::{InspectError, NativeImage, format::page::PAGE_BYTES, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

/// Check immutable captured bytes against the append-table source profile.
/// Unsupported lengths refuse before source capture or any allocation. The
/// caller issues the catalog scope used for a qualified-size capture.
/// No proposed name, synthetic declaration, output plan, clock or write is used.
/// Classified write-source/profile or structural/constraint refusals return false;
/// false is not a database-health judgment. Cancellation, allocation,
/// budget, I/O and canonical-construction failures remain errors. This advisory
/// fact never grants permission to apply: actual planning captures and rechecks
/// its source and validates the real requested name and declaration separately.
pub fn add_table_source_available(
    captured: &[u8],
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    let _span = tracing::debug_span!("native.add_table.source_available").entered();
    check(budget, cancelled)?;
    if captured.len() != source::OLD_PAGES * PAGE_BYTES {
        return Ok(false);
    }
    let result = (|| {
        let image = NativeImage::capture(&mut Cursor::new(captured), scope, budget, cancelled)?;
        source::qualify(&image, budget, cancelled).map(|_| ())
    })();
    match result {
        Ok(()) => Ok(true),
        Err(AddTableError::Source) => Ok(false),
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
        )) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;

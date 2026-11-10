//! Positive empty permission/relationship storage for current native table DROP.
use super::{definition, ledger::Ledger};
use crate::{EditError, NativeImage, WriteBlocker, header::check, table::mutation::schema_source};
use tablecraft_core::limits::ResourceBudget;

/// Validate actual system declarations and both native maps, not catalog names alone.
/// The caller independently qualifies their ancestry and the complete application graph.
pub(in crate::table) fn require_empty(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut ledger = Ledger::new(image, budget, cancelled)?;
    for (name, role) in [
        ("MSysRelationships", definition::Role::Relationships),
        ("MSysACEs", definition::Role::Aces),
    ] {
        check(budget, cancelled)?;
        let origin =
            schema_source::find_role(image, name, budget, cancelled).map_err(
                |error| match error {
                    schema_source::Error::Inspect(error) => EditError::Inspect(error),
                    schema_source::Error::Source | schema_source::Error::Dependency => {
                        EditError::Unsupported(WriteBlocker::ActiveContent)
                    }
                },
            )?;
        let number = u32::try_from(origin.native_id())
            .map_err(|_| EditError::Unsupported(WriteBlocker::ActiveContent))?;
        let table = definition::capture(image, &mut ledger, number, role, budget, cancelled)?;
        if table.layout.records != 0 || !table.owned.is_empty(budget, cancelled)? {
            return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
        }
    }
    Ok(())
}

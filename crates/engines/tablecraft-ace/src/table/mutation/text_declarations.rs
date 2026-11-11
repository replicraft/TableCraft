//! Empty Text writes require an actual policy declaration, not an inferred default.
use crate::{
    EditError, NativeColumn, PropertyGroupKind, TableProperties, TableSchema, WriteBlocker,
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

const ALLOW_ZERO_LENGTH: &str = "AllowZeroLength";

/// The source must already qualify stored property types and constraint semantics.
/// Presence is required here: explicit false remains a normal constraint violation,
/// while an absent declaration supplies no authority to store empty Text.
pub(super) fn require(
    properties: &TableProperties,
    schema: &TableSchema,
    column: &NativeColumn,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    if properties.object_id() != schema.object_id() {
        return Err(EditError::Preservation);
    }
    for group in properties.groups() {
        check(budget, cancelled)?;
        if group.kind() != PropertyGroupKind::Column
            || !group.name().eq_ignore_ascii_case(column.name())
        {
            continue;
        }
        for property in group.properties() {
            check(budget, cancelled)?;
            if property.name().eq_ignore_ascii_case(ALLOW_ZERO_LENGTH) {
                return Ok(());
            }
        }
        // Match the constraint reader's first-group lookup exactly; a later
        // similarly named group cannot supply missing declaration authority.
        break;
    }
    Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
}

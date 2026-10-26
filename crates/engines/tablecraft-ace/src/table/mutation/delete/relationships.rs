//! Closed DELETE rule admission after the complete existing before-image gates.
use crate::{EditError, NativeImage, WriteBlocker, header::check, inspect_relationships};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ReferentialAction;

pub(super) fn qualify(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // The caller must first complete before_delete: reciprocal/catalog checks,
    // stored membership and the existing mutation-carrier allowlist remain
    // independent requirements. This metadata pass alone grants no mutation.
    let scope = image.catalog().catalog().scope();

    for relationship in inspect_relationships(&mut image.reader(), scope, budget, cancelled)? {
        check(budget, cancelled)?;

        let declaration = relationship.declaration();
        let parent = declaration.parent_table().as_str();
        let child = declaration.child_table().as_str();

        if !parent.eq_ignore_ascii_case(table) && !child.eq_ignore_ascii_case(table) {
            continue;
        }

        let rules = declaration
            .rules()
            .ok_or(EditError::Unsupported(WriteBlocker::ConstraintCoverage))?;

        if !rules.enforced
            || rules.on_delete != ReferentialAction::Restrict
            || !matches!(
                rules.on_update,
                ReferentialAction::Restrict | ReferentialAction::Cascade
            )
            || parent.eq_ignore_ascii_case(child)
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
    }

    // No keys are rewritten or actions dispatched. Complete post-edit FK
    // membership rejects a removed parent with any surviving dependent row.
    Ok(())
}

//! Shared immutable proof for the five distinct original INSERT families.
use crate::transaction::qualified_recovery::equal_image;
use crate::{NativeEditPlan, NativeRecoveryQualificationError, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn finish(
    original: &NativeEditPlan,
    rebuilt: NativeEditPlan,
    proof: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, NativeRecoveryQualificationError> {
    equal_image(
        rebuilt.before_bytes(),
        original.before_bytes(),
        proof,
        cancelled,
    )?;
    equal_image(
        rebuilt.after_bytes(),
        original.after_bytes(),
        proof,
        cancelled,
    )?;
    if rebuilt.changed_rows() != original.changed_rows() {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    check(proof, cancelled)?;
    Ok(rebuilt)
}

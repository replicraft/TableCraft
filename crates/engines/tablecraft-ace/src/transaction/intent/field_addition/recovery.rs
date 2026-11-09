//! Same retained V2 session after complete typed-source/target reconstruction.
use super::{
    FieldAdditionIntentError as Error, NativeFieldAdditionIntent as Intent, planning::same_image,
};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// Original checked V2 session after exact source/history/declaration proof.
#[must_use = "recover explicitly or retain the qualified typed-field journal"]
pub struct QualifiedFieldAdditionRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Rebuild exact BEFORE/AFTER; neither the decoded request nor generic IO AFTER
/// grants native authority. Source binding, canonical history reconstruction and
/// every complete image comparison spend the SAME caller cumulative allowance.
pub fn qualify_native_field_addition_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedFieldAdditionRecovery<'a, J>, Error> {
    let _span = tracing::debug_span!("native.field_addition.recovery_qualify").entered();
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let candidate = intent.reconstruct(image, budget, cancelled)?;
    same_image(
        candidate.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    same_image(
        candidate.after_bytes(),
        checked.after_bytes(),
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(QualifiedFieldAdditionRecovery { checked })
}

impl<J: DurableStorage> QualifiedFieldAdditionRecovery<'_, J> {
    /// Framing state of this exact retained V2 journal.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Explicitly retain/restore the paired complete bytes and length.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify a settled target with barriers, without inventing repair authority.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}

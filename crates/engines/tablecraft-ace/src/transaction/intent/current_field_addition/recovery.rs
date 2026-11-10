//! Checked V2 recovery after current source and complete image qualification.
mod retained;
pub use retained::QualifiedRetainedFieldAdditionRecovery;

#[cfg(feature = "ace16-create")]
use super::{
    planning::same_image,
    record::{
        CurrentFieldAdditionIntentError as Error, NativeCurrentFieldAdditionIntent as Intent,
    },
};
#[cfg(feature = "ace16-create")]
use crate::{NativeImage, header::check};
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
#[cfg(feature = "ace16-create")]
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "ace16-create")]
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
#[cfg(feature = "ace16-create")]
use tablecraft_model::CatalogScope;

/// Original checked V2 session after exact current-source and declaration proof.
#[must_use = "recover explicitly or retain the qualified typed-field journal"]
#[cfg(feature = "ace16-create")]
pub struct QualifiedCurrentFieldAdditionRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Rebuild exact BEFORE/AFTER; neither the decoded request nor generic IO AFTER
/// grants native authority. Source binding, current metadata qualification and
/// every complete image comparison spend the SAME caller cumulative allowance.
#[cfg(feature = "ace16-create")]
pub fn qualify_native_current_field_addition_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedCurrentFieldAdditionRecovery<'a, J>, Error> {
    let _span = tracing::debug_span!("native.current_field_addition.recovery_qualify").entered();
    check(budget, cancelled)?;
    if intent.is_retained() {
        return Err(Error::InvalidRecord);
    }
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
    Ok(QualifiedCurrentFieldAdditionRecovery { checked })
}

#[cfg(feature = "ace16-create")]
impl<J: DurableStorage> QualifiedCurrentFieldAdditionRecovery<'_, J> {
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
